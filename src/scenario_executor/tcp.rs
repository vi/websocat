use std::{net::SocketAddr, time::Duration};

use crate::scenario_executor::utils1::{wrap_as_stream_socket, TaskHandleExt2};
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::net::TcpStream;
use tracing::{debug, debug_span, error, Instrument};

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

fn listen_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("listen_tcp");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpListenOpts {
        addr: SocketAddr,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,
    }
    let opts: TcpListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=%opts.addr, "options parsed");

    let autospawn = opts.autospawn;

    Ok(async move {
        debug!("node started");
        let l = tokio::net::TcpListener::bind(opts.addr).await?;

        let mut drop_nofity = None;

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
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
                            continuation,
                            (h, from),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<StreamSocket>, SocketAddr)>(
                                the_scenario,
                                continuation,
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
