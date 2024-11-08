use std::{ffi::OsString, time::Duration};

use crate::scenario_executor::utils::{SimpleErr, TaskHandleExt2};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::net::UnixStream;
use tracing::{debug, debug_span, error, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

use super::utils::RhResult;

//@ Connect to a UNIX stream socket of some kind
fn connect_unix(
    ctx: NativeCallContext,
    opts: Dynamic,
    path: OsString,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_unix");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct UnixOpts {
        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,
    }
    let opts: UnixOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, ?path, r#abstract=opts.r#abstract, "options parsed");

    Ok(async move {
        debug!("node started");
        let t = UnixStream::connect(path).await?;
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

fn listen_unix(
    ctx: NativeCallContext,
    opts: Dynamic,
    path: OsString,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("listen_tcp");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpListenOpts {
        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,
    }
    let opts: TcpListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=?path, r#abstract=opts.r#abstract, "options parsed");

    let autospawn = opts.autospawn;

    Ok(async move {
        debug!("node started");
        let l = tokio::net::UnixListener::bind(path)?;

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.accept().await {
                Ok((t, from)) => {
                    let newspan = debug_span!("unix_accept", from=?from);
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
                        callback_and_continue::<(Handle<StreamSocket>,)>(
                            the_scenario,
                            continuation,
                            (h,),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<StreamSocket>,)>(
                                the_scenario,
                                continuation,
                                (h,),
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
        }
    }
    .instrument(span)
    .wrap())
}

fn unlink_file(
    ctx: NativeCallContext,
    path: OsString,
    //@ Emit error if unlinking fails.
    bail_if_fails: bool,
) -> RhResult<()> {
    match std::fs::remove_file(&path) {
        Ok(_) => {
            debug!(?path, "Unlinked file");
            Ok(())
        }
        Err(e) => {
            if bail_if_fails {
                warn!(?path, %e, "Failed to unlink");
                Err(ctx.err("failed to unlink"))
            } else {
                debug!(?path, %e, "Failed to unlink");
                Ok(())
            }
        }
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_unix", connect_unix);
    engine.register_fn("listen_unix", listen_unix);
    engine.register_fn("unlink_file", unlink_file);
}
