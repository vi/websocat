use std::{net::SocketAddr, time::Duration};

use crate::scenario_executor::{
    utils1::{wrap_as_stream_socket, SimpleErr, TaskHandleExt2},
    utils2::AddressOrFd,
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
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addr=%opts.addr, "options parsed");

    Ok(async move {
        debug!("node started");
        let t = TcpStream::connect(opts.addr).await?;
        let (r, w) = t.into_split();
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
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
    struct TcpOpts {}
    let _opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addrs=?addrs, "options parsed");

    Ok(async move {
        debug!("node started");

        let mut fu = FuturesUnordered::new();

        for addr in addrs {
            fu.push(TcpStream::connect(addr).map(move |x| (x, addr)));
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
                    anyhow::bail!("failed to connect to any of the candidates")
                }
            }
        };

        let (r, w) = t.into_split();
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
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
        //@ Socket address to bind listening socket tp
        addr: Option<SocketAddr>,

        //@ Inherited file descriptor to
        fd: Option<i32>,

        //@ Skip socket type check when using `fd`.
        #[serde(default)]
        fd_force: bool,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    if !(opts.addr.is_some() ^ opts.fd.is_some()) {
        return Err(ctx.err("Exactly one of `addr` or `fd` must be specified"));
    }

    let a = if let Some(x) = opts.addr {
        debug!(parent: &span, listen_addr=%x, "options parsed");
        AddressOrFd::Addr(x)
    } else if let Some(x) = opts.fd {
        debug!(parent: &span, fd=%x, "options parsed");
        AddressOrFd::Fd(x)
    } else {
        unreachable!()
    };

    let autospawn = opts.autospawn;

    Ok(async move {
        debug!("node started");
        let l = match a {
            AddressOrFd::Addr(a) => tokio::net::TcpListener::bind(a).await?,
            #[cfg(not(unix))]
            AddressOrFd::Fd(f) => {
                error!("Inheriting listeners from parent processes is not supported outside UNIX platforms");
                anyhow::bail!("Unsupported feature");
            }
            #[cfg(unix)]
            AddressOrFd::Fd(f) => {
                use super::unix::{listen_from_fd,ListenFromFdOutcome,ListenFromFdType};
                match unsafe { listen_from_fd(f, opts.fd_force.then_some(ListenFromFdType::Tcp))}? {
                    ListenFromFdOutcome::Tcp(x) => x,
                    x => {
                        error!("File descriptor {f} has invalid socket type: {x:?}");
                        anyhow::bail!("Unsupported feature");
                    }
                }
            }
        };

        let mut address_to_report = opts.addr.unwrap_or(SocketAddr::V4(std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, 0)));

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
                    let (r, w) = t.into_split();

                    let (s, dn) = wrap_as_stream_socket(r, w, None, opts.oneshot);
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
