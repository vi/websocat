use std::{net::SocketAddr, time::Duration};

use crate::scenario_executor::utils::TaskHandleExt2;
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, error, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

fn connect_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
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
        let t = tokio::net::TcpStream::connect(opts.addr).await?;
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

        callback_and_continue(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

fn listen_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let span = debug_span!("listen_tcp");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpListenOpts {
        addr: SocketAddr,
        #[serde(default)]
        autospawn: bool,
    }
    let opts: TcpListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=%opts.addr, "options parsed");

    let autospawn = opts.autospawn;

    Ok(async move {
        debug!("node started");
        let l = tokio::net::TcpListener::bind(opts.addr).await?;

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.accept().await {
                Ok((t, from)) => {
                    let newspan = debug_span!("tcp_accept", from=%from);
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

                    debug!(parent: &newspan, s=?s,"accepted");
                    let h = s.wrap();
                    if !autospawn {
                        callback_and_continue(the_scenario, continuation, (h, from))
                            .instrument(newspan)
                            .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue(the_scenario, continuation, (h, from))
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
        }
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
    engine.register_fn("listen_tcp", listen_tcp);
}
