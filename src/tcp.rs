use std::{net::SocketAddr, time::Duration};

use crate::utils::{Anyhow2EvalAltResult, TaskHandleExt2};
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, error, field, Instrument};

use crate::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

fn connect_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_tcp", addr = field::Empty);
    let the_scenario = ctx.get_scenario().tbar()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    span.record("addr", field::display(opts.addr));
    debug!(parent: &span, "options parsed");

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

        callback_and_continue(the_scenario, continuation, (h,))
            .await;
        Ok(())
    }.instrument(span)
    .wrap())
}

fn listen_tcp(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let span = debug_span!("listen_tcp", addr = field::Empty);
    let the_scenario = ctx.get_scenario().tbar()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpListenOpts {
        addr: SocketAddr,
    }
    let opts: TcpListenOpts = rhai::serde::from_dynamic(&opts)?;
    span.record("addr", field::display(opts.addr));
    debug!(parent: &span, "options parsed");

    Ok(async move {
        debug!("node started");
        let l = tokio::net::TcpListener::bind(opts.addr).await?;

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.accept().await {
                Ok((t, from)) => {
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

                    debug!(s=?s, from=?from, "accepted");
                    let h = s.wrap();
                    callback_and_continue(the_scenario, continuation, (h,from,))
                        .await;
                }
                Err(e) => {
                    error!("Error from accept: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }.instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
    engine.register_fn("listen_tcp", listen_tcp);
}
