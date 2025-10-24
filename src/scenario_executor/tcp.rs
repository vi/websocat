use std::{net::SocketAddr, pin::Pin, time::Duration};

use crate::{
    copy_common_tcp_bind_options, copy_common_tcp_stream_options,
    scenario_executor::{
        exit_code::EXIT_CODE_TCP_CONNECT_FAIL,
        socketopts::{TcpBindOptions, TcpStreamOptions},
        utils1::{NEUTRAL_SOCKADDR4, TaskHandleExt2, wrap_as_stream_socket},
        utils2::AddressOrFd,
    },
};
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::net::TcpStream;
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

        //@ Set IP_TRANSPARENT for the socket
        #[serde(default)]
        transparent: bool,

        //@ Set IP_FREEBIND for the socket
        #[serde(default)]
        freebind: bool,

        //@ Set IPV6_TCLASS for the socket, in case when it is IPv6.
        tclass_v6: Option<u32>,

        //@ Set IP_TOS for the socket, in case when it is IPv4.
        tos_v4: Option<u32>,

        //@ Set IP_TTL for a IPv4 socket or IPV6_UNICAST_HOPS for an IPv6 socket
        ttl: Option<u32>,

        //@ Set SO_LINGER for the socket
        linger_s: Option<u32>,

        //@ Set SO_OOBINLINE for the socket
        #[serde(default)]
        out_of_band_inline: bool,

        //@ Set IPV6_V6ONLY for the socket in case when it is IPv6
        only_v6: Option<bool>,

        //@ Set TCP_NODELAY (no Nagle) for the socket
        nodelay: Option<bool>,

        //@ Set TCP_CONGESTION for the socket
        tcp_congestion: Option<String>,

        //@ Set SO_INCOMING_CPU for the socket
        cpu_affinity: Option<usize>,

        //@ Set TCP_USER_TIMEOUT for the socket
        user_timeout_s: Option<u32>,

        //@ Set SO_PRIORITY for the socket
        priority: Option<u32>,

        //@ Set SO_RCVBUF for the socket
        recv_buffer_size: Option<usize>,

        //@ Set SO_SNDBUF for the socket
        send_buffer_size: Option<usize>,

        //@ Set TCP_MAXSEG for the socket
        mss: Option<u32>,

        //@ Set SO_MARK for the socket
        mark: Option<u32>,

        //@ Set TCP_THIN_LINEAR_TIMEOUTS for the socket
        thin_linear_timeouts: Option<bool>,

        //@ Set TCP_NOTSENT_LOWAT for the socket
        notsent_lowat: Option<u32>,

        //@ Set SO_KEEPALIVE for the socket
        keepalive: Option<bool>,

        //@ Set TCP_KEEPCNT for the socket
        keepalive_retries: Option<u32>,

        //@ Set TCP_KEEPINTVL for the socket
        keepalive_interval_s: Option<u32>,

        //@ Set TCP_KEEPALIVE for the socket
        keepalive_idletime_s: Option<u32>,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addr=%opts.addr, "options parsed");

    let mut tcpbindopts = TcpBindOptions::new();
    let mut tcpstreamopts = TcpStreamOptions::new();
    tcpbindopts.bind_before_connecting = opts.bind;
    copy_common_tcp_bind_options!(tcpbindopts, opts);
    copy_common_tcp_stream_options!(tcpstreamopts, opts);

    Ok(async move {
        debug!("node started");
        let t = tcpbindopts
            .connect(opts.addr, &tcpstreamopts)
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

        //@ Set SO_REUSEADDR for the listening socket
        reuseaddr: Option<bool>,

        //@ Set SO_REUSEPORT for the listening socket
        #[serde(default)]
        reuseport: bool,

        //@ Set SO_BINDTODEVICE for the listening socket
        bind_device: Option<String>,

        //@ Set IP_TRANSPARENT for the listening socket
        #[serde(default)]
        transparent: bool,

        //@ Set IP_FREEBIND for the listening socket
        #[serde(default)]
        freebind: bool,

        //@ Set IPV6_V6ONLY for the socket in case when it is IPv6
        only_v6: Option<bool>,

        //@ Set IPV6_TCLASS for the socket, in case when it is IPv6.
        tclass_v6: Option<u32>,

        //@ Set IP_TOS for the socket, in case when it is IPv4.
        tos_v4: Option<u32>,

        //@ Set IP_TTL for a IPv4 socket or IPV6_UNICAST_HOPS for an IPv6 socket
        ttl: Option<u32>,

        //@ Set SO_LINGER for the socket
        linger_s: Option<u32>,

        //@ Set SO_OOBINLINE for the socket
        #[serde(default)]
        out_of_band_inline: bool,

        //@ Set TCP_NODELAY (no Nagle) for the socket
        nodelay: Option<bool>,

        //@ Set TCP_CONGESTION for the socket
        tcp_congestion: Option<String>,

        //@ Set SO_INCOMING_CPU for the socket
        cpu_affinity: Option<usize>,

        //@ Set TCP_USER_TIMEOUT for the socket
        user_timeout_s: Option<u32>,

        //@ Set SO_PRIORITY for the socket
        priority: Option<u32>,

        //@ Set SO_RCVBUF for the socket
        recv_buffer_size: Option<usize>,

        //@ Set SO_SNDBUF for the socket
        send_buffer_size: Option<usize>,

        //@ Set TCP_MAXSEG for the socket
        mss: Option<u32>,

        //@ Set SO_MARK for the socket
        mark: Option<u32>,

        //@ Set TCP_THIN_LINEAR_TIMEOUTS for the socket
        thin_linear_timeouts: Option<bool>,

        //@ Set TCP_NOTSENT_LOWAT for the socket
        notsent_lowat: Option<u32>,

        //@ Set SO_KEEPALIVE for the socket
        keepalive: Option<bool>,

        //@ Set TCP_KEEPCNT for the socket
        keepalive_retries: Option<u32>,

        //@ Set TCP_KEEPINTVL for the socket
        keepalive_interval_s: Option<u32>,

        //@ Set TCP_KEEPALIVE for the socket
        keepalive_idletime_s: Option<u32>,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addrs=?addrs, "options parsed");

    let mut tcpbindopts = TcpBindOptions::new();
    let mut tcpstreamopts = TcpStreamOptions::new();
    tcpbindopts.bind_before_connecting = opts.bind;
    copy_common_tcp_bind_options!(tcpbindopts, opts);
    copy_common_tcp_stream_options!(tcpstreamopts, opts);

    Ok(async move {
        debug!("node started");

        if addrs.is_empty() {
            anyhow::bail!("No addresses to connect TCP to");
        }

        let mut fu = FuturesUnordered::new();

        for addr in addrs {
            fu.push(
                tcpbindopts
                    .connect(addr, &tcpstreamopts)
                    .map(move |x| (x, addr)),
            );
        }

        let mut first_error = None;

        let t: TcpStream = loop {
            match fu.next().await {
                Some((Ok(x), addr)) => {
                    debug!(%addr, "connected");
                    break x;
                }
                Some((Err(e), addr)) => {
                    debug!(%addr, %e, "failed to connect");
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
                None => {
                    the_scenario.exit_code.set(EXIT_CODE_TCP_CONNECT_FAIL);
                    return Err(first_error.expect("Empty set should be handled above").into());
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

        //@ Set SO_REUSEADDR for the listening socket
        reuseaddr: Option<bool>,

        //@ Set SO_REUSEPORT for the listening socket
        #[serde(default)]
        reuseport: bool,

        //@ Set SO_BINDTODEVICE for the listening socket
        bind_device: Option<String>,

        //@ Set size of the queue of unaccepted pending connections for this socket.
        //@ Default is 1024 when `oneshot` is off and 1 and `oneshot` is on.
        backlog: Option<u32>,

        //@ Set IP_TRANSPARENT for the listening socket
        #[serde(default)]
        transparent: bool,

        //@ Set IP_FREEBIND for the listening socket
        #[serde(default)]
        freebind: bool,

        //@ Set IPV6_V6ONLY for the listening socket
        only_v6: Option<bool>,

        //@ Set IPV6_TCLASS for accepted IPv6 sockets
        tclass_v6: Option<u32>,

        //@ Set IP_TOS for accepted IPv4 sockets
        tos_v4: Option<u32>,

        //@ Set IP_TTL accepted IPv4 sockets and or IPV6_UNICAST_HOPS for an IPv6
        ttl: Option<u32>,

        //@ Set SO_LINGER for accepted sockets
        linger_s: Option<u32>,

        //@ Set SO_OOBINLINE for accepted sockets
        #[serde(default)]
        out_of_band_inline: bool,

        //@ Set TCP_NODELAY (no Nagle) for accepted sockets
        nodelay: Option<bool>,

        //@ Set TCP_CONGESTION for accepted sockets
        tcp_congestion: Option<String>,

        //@ Set SO_INCOMING_CPU for accepted sockets
        cpu_affinity: Option<usize>,

        //@ Set TCP_USER_TIMEOUT for accepted sockets
        user_timeout_s: Option<u32>,

        //@ Set SO_PRIORITY for accepted sockets
        priority: Option<u32>,

        //@ Set SO_RCVBUF for accepted sockets
        recv_buffer_size: Option<usize>,

        //@ Set SO_SNDBUF for accepted sockets
        send_buffer_size: Option<usize>,

        //@ Set TCP_MAXSEG for accepted sockets
        mss: Option<u32>,

        //@ Set SO_MARK for accepted sockets
        mark: Option<u32>,

        //@ Set TCP_THIN_LINEAR_TIMEOUTS for accepted sockets
        thin_linear_timeouts: Option<bool>,

        //@ Set TCP_NOTSENT_LOWAT for accepted sockets
        notsent_lowat: Option<u32>,

        //@ Set SO_KEEPALIVE for accepted sockets
        keepalive: Option<bool>,

        //@ Set TCP_KEEPCNT for accepted sockets
        keepalive_retries: Option<u32>,

        //@ Set TCP_KEEPINTVL for accepted sockets
        keepalive_interval_s: Option<u32>,

        //@ Set TCP_KEEPALIVE for accepted sockets
        keepalive_idletime_s: Option<u32>,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let a = AddressOrFd::interpret(&ctx, &span, opts.addr, opts.fd, opts.named_fd, None)?;

    let autospawn = opts.autospawn;

    let mut tcpbindopts = TcpBindOptions::new();
    let mut tcpstreamopts = TcpStreamOptions::new();
    copy_common_tcp_bind_options!(tcpbindopts, opts);
    copy_common_tcp_stream_options!(tcpstreamopts, opts);
    if let Some(bklg) = opts.backlog {
        tcpbindopts.listen_backlog = bklg;
    } else if opts.oneshot {
        tcpbindopts.listen_backlog = 1;
    } else {
        tcpbindopts.listen_backlog = 1024;
    }

    Ok(async move {
        debug!("node started");
        
        let mut address_to_report = *a.addr().unwrap_or(&NEUTRAL_SOCKADDR4);

        let l = match a {
            AddressOrFd::Addr(a) => tcpbindopts.bind(a).await?,
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

                    debug!(parent: &newspan, "begin accept");
                    
                    #[allow(unused_assignments)]
                    let mut fd = None;
                    #[cfg(unix)]
                    {
                        use std::os::fd::AsRawFd;
                        fd = Some(
                            // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                            unsafe{super::types::SocketFd::new(t.as_raw_fd())});
                    }

                    tcpstreamopts.apply_socket_opts(&t, from.is_ipv6())?;

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
