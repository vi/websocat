use std::{net::SocketAddr, pin::Pin, time::Duration};

use crate::scenario_executor::{
    exit_code::EXIT_CODE_TCP_CONNECT_FAIL,
    utils1::{wrap_as_stream_socket, TaskHandleExt2, NEUTRAL_SOCKADDR4},
    utils2::AddressOrFd,
};
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tracing::{debug, debug_span, error, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

use super::utils1::RhResult;

/// Control of TCP (or other sort of) socket may be suddenly yanked away,  e.g. using `--exec-dup`, so automatic shutdowns
/// are not our friends. Just `close(2)` things when dropped without extra steps.
struct TcpOwnedWriteHalfWithoutAutoShutdown(Option<tokio::net::tcp::OwnedWriteHalf>);

impl tokio::io::AsyncWrite for TcpOwnedWriteHalfWithoutAutoShutdown {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0.as_mut().unwrap()).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0.as_mut().unwrap()).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0.as_mut().unwrap()).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0.as_mut().unwrap()).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.as_ref().unwrap().is_write_vectored()
    }
}
impl Drop for TcpOwnedWriteHalfWithoutAutoShutdown {
    fn drop(&mut self) {
        self.0.take().unwrap().forget();
    }
}
impl TcpOwnedWriteHalfWithoutAutoShutdown {
    fn new(w: tokio::net::tcp::OwnedWriteHalf) -> Self {
        Self(Some(w))
    }
}

struct TcpBindOptions {
    bind_before_connecting: Option<SocketAddr>,
    reuseaddr: Option<bool>,
    reuseport: bool,
    bind_device: Option<String>,
    listen_backlog: u32,
}

macro_rules! cfg_gated_block_or_err {
    ($feature:literal, #[cfg($($c:tt)*)], $b:block$ (,)?) => {
        'a: { 
            #[cfg($($c)*)] {
                $b;
                break 'a;
            }
            #[allow(unreachable_code)]
            return Err(std::io::Error::new(std::io::ErrorKind::Other, concat!("Not supported on this platform: `",$feature,"`")))
        }
    };
}

macro_rules! copy_common_tcp_bind_options {
    ($target:ident, $source:ident) => {
        $target.reuseaddr = $source.reuseaddr;
        $target.reuseport = $source.reuseport;
        $target.bind_device = $source.bind_device;
    };
}

impl TcpBindOptions {
    fn new() -> TcpBindOptions {
        Self {
            bind_before_connecting: None,
            reuseaddr: None,
            reuseport: false,
            bind_device: None,
            listen_backlog: 1024,
        }
    }

    fn gs4a(addr: SocketAddr) -> std::io::Result<TcpSocket> {
        if addr.is_ipv4() {
            TcpSocket::new_v4()
        } else if addr.is_ipv6() {
            TcpSocket::new_v6()
        } else {
            panic!("Non IPv4 or IPv6 address is specified for a TCP socket");
        }
    }

    fn setopts(&self, s: &TcpSocket, pending_listen: bool) -> std::io::Result<()> {
        if let Some(v) = self.reuseaddr {
            s.set_reuseaddr(v)?;
        } else {
            if pending_listen {
                #[cfg(not(windows))]
                s.set_reuseaddr(true)?;
            }
        }
        if self.reuseport {
            cfg_gated_block_or_err!(
                "reuseport",
                #[cfg(all(
                    unix,
                    not(target_os = "solaris"),
                    not(target_os = "illumos"),
                    not(target_os = "cygwin"),
                ))],
                {
                    s.set_reuseport(true)?;
                },
            );
        }
        if let Some(ref v) = self.bind_device {
            cfg_gated_block_or_err!(
                "bind_device",
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                {
                    s.bind_device(Some(v[..].as_bytes()))?;
                },
            );
        }
        Ok(())
    }

    async fn connect(&self, addr: SocketAddr) -> std::io::Result<TcpStream> {
        let s = Self::gs4a(addr)?;
        self.setopts(&s, false)?;
        if let Some(bbc) = self.bind_before_connecting {
            s.bind(bbc)?;
        }
        s.connect(addr).await
        //TcpStream::connect(addr).await
    }


    async fn bind(&self, addr: SocketAddr) -> std::io::Result<TcpListener> {
        let s = Self::gs4a(addr)?;
        self.setopts(&s, true)?;
        s.bind(addr)?;
        s.listen(self.listen_backlog)
        //TcpListener::bind(addr).await
    }
}

fn connect_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {   
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_tcp");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,

        //@ Bind TCP socket to this address and/or port before issuing `connect`
        bind: Option<SocketAddr>,

        //@ Set SO_REUSEADDR for the socket
        reuseaddr: Option<bool>,

        //@ Set SO_REUSEPORT for the socket
        #[serde(default)]
        reuseport: bool,

        //@ Set SO_BINDTODEVICE for the socket
        bind_device: Option<String>,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addr=%opts.addr, "options parsed");

    let mut tcpopts = TcpBindOptions::new();
    tcpopts.bind_before_connecting = opts.bind;
    copy_common_tcp_bind_options!(tcpopts, opts);

    Ok(async move {
        debug!("node started");
        let t = tcpopts
            .connect(opts.addr)
            .await
            .inspect_err(|_| the_scenario.exit_code.set(EXIT_CODE_TCP_CONNECT_FAIL))?;
        #[allow(unused_assignments)]
        let mut fd = None;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            fd = Some(
                // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                unsafe { super::types::SocketFd::new(t.as_raw_fd()) },
            );
        }
        let (r, w) = t.into_split();
        let w = TcpOwnedWriteHalfWithoutAutoShutdown::new(w);
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
            fd,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

fn connect_tcp_race(
    ctx: NativeCallContext,
    opts: Dynamic,
    addrs: Vec<SocketAddr>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_tcp_race");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpOpts {

        //@ Bind TCP socket to this address and/or port before issuing `connect`
        bind: Option<SocketAddr>,

        //@ Set SO_REUSEADDR for the socket
        reuseaddr: Option<bool>,

        //@ Set SO_REUSEPORT for the socket
        #[serde(default)]
        reuseport: bool,

        //@ Set SO_BINDTODEVICE for the socket
        bind_device: Option<String>,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addrs=?addrs, "options parsed");

    let mut tcpopts = TcpBindOptions::new();
    tcpopts.bind_before_connecting = opts.bind;
    copy_common_tcp_bind_options!(tcpopts, opts);

    Ok(async move {
        debug!("node started");

        let mut fu = FuturesUnordered::new();

        for addr in addrs {
            fu.push(tcpopts.connect(addr).map(move |x| (x, addr)));
        }

        let t: TcpStream = loop {
            match fu.next().await {
                Some((Ok(x), addr)) => {
                    debug!(%addr, "connected");
                    break x;
                }
                Some((Err(e), addr)) => {
                    debug!(%addr, %e, "failed to connect");
                }
                None => {
                    the_scenario.exit_code.set(EXIT_CODE_TCP_CONNECT_FAIL);
                    anyhow::bail!("failed to connect to any of the candidates")
                }
            }
        };

        #[allow(unused_assignments)]
        let mut fd = None;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            fd = Some(
                // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                unsafe { super::types::SocketFd::new(t.as_raw_fd()) },
            );
        }

        let (r, w) = t.into_split();
        let w = TcpOwnedWriteHalfWithoutAutoShutdown::new(w);
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
            fd,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Listen TCP socket at specified address
fn listen_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Called once after the port is bound
    when_listening: FnPtr,
    //@ Called on each connection
    on_accept: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("listen_tcp");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Socket address to bind listening socket to
        addr: Option<SocketAddr>,

        //@ Inherited file descriptor to accept connections from
        fd: Option<i32>,

        //@ Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
        named_fd: Option<String>,

        //@ Skip socket type check when using `fd`.
        #[serde(default)]
        fd_force: bool,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,

        //@ Set SO_REUSEADDR for the socket
        reuseaddr: Option<bool>,

        //@ Set SO_REUSEPORT for the socket
        #[serde(default)]
        reuseport: bool,

        //@ Set SO_BINDTODEVICE for the socket
        bind_device: Option<String>,

        //@ Set size of the queue of unaccepted pending connections for this socket.
        backlog: Option<u32>,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let a = AddressOrFd::interpret(&ctx, &span, opts.addr, opts.fd, opts.named_fd, None)?;

    let autospawn = opts.autospawn;

    let mut tcpopts = TcpBindOptions::new();
    copy_common_tcp_bind_options!(tcpopts, opts);
    if let Some(bklg) = opts.backlog {
        tcpopts.listen_backlog = bklg;
    } else {
        if opts.oneshot {
            tcpopts.listen_backlog = 1;
        } else {
            tcpopts.listen_backlog = 1024;
        }
    }

    Ok(async move {
        debug!("node started");
        
        let mut address_to_report = *a.addr().unwrap_or(&NEUTRAL_SOCKADDR4);

        let l = match a {
            AddressOrFd::Addr(a) => tcpopts.bind(a).await?,
            #[cfg(not(unix))]
            AddressOrFd::Fd(..) | AddressOrFd::NamedFd(..) => {
                error!("Inheriting listeners from parent processes is not supported outside UNIX platforms");
                anyhow::bail!("Unsupported feature");
            }
            #[cfg(unix)]
            AddressOrFd::Fd(f) => {
                use super::unix1::{listen_from_fd,ListenFromFdType};
                unsafe{listen_from_fd(f, opts.fd_force.then_some(ListenFromFdType::Tcp), Some(ListenFromFdType::Tcp))}?.unwrap_tcp()
            }
            #[cfg(unix)]
            AddressOrFd::NamedFd(f) => {
                use super::unix1::{listen_from_fd_named,ListenFromFdType};
                unsafe{listen_from_fd_named(&f, opts.fd_force.then_some(ListenFromFdType::Tcp), Some(ListenFromFdType::Tcp))}?.unwrap_tcp()
            }
        };

        if address_to_report.port() == 0 {
            if let Ok(a) = l.local_addr() {
                address_to_report = a;
            } else {
                warn!("Failed to obtain actual listening port");
            }
        }

        callback_and_continue::<(SocketAddr,)>(
            the_scenario.clone(),
            when_listening,
            (address_to_report,),
        )
        .await;

        let mut drop_nofity = None;

        loop {
            let the_scenario = the_scenario.clone();
            let on_accept = on_accept.clone();
            match l.accept().await {
                Ok((t, from)) => {
                    let newspan = debug_span!("tcp_accept", from=%from);
                    
                    #[allow(unused_assignments)]
                    let mut fd = None;
                    #[cfg(unix)]
                    {
                        use std::os::fd::AsRawFd;
                        fd = Some(
                            // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                            unsafe{super::types::SocketFd::new(t.as_raw_fd())});
                    }

                    let (r, w) = t.into_split();
                    let w = TcpOwnedWriteHalfWithoutAutoShutdown::new(w);

                    let (s, dn) = wrap_as_stream_socket(r, w, None, fd, opts.oneshot);
                    drop_nofity = dn;

                    debug!(parent: &newspan, s=?s,"accepted");

                    let h = s.wrap();

                    if !autospawn {
                        callback_and_continue::<(Handle<StreamSocket>, SocketAddr)>(
                            the_scenario,
                            on_accept,
                            (h, from),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<StreamSocket>, SocketAddr)>(
                                the_scenario,
                                on_accept,
                                (h, from),
                            )
                            .instrument(newspan)
                            .await;
                        });
                    }
                }
                Err(e) => {
                    error!("Error from accept: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
            if opts.oneshot {
                debug!("Exiting TCP listener due to --oneshot mode");
                break;
            }
        }

        if let Some((dn1, dn2)) = drop_nofity {
            debug!("Waiting for the sole accepted client to finish serving reads");
            let _ = dn1.await;
            debug!("Waiting for the sole accepted client to finish serving writes");
            let _ = dn2.await;
            debug!("The sole accepted client finished");
        }
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
    engine.register_fn("connect_tcp_race", connect_tcp_race);
    engine.register_fn("listen_tcp", listen_tcp);
}
