use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, error, Instrument};

use crate::scenario_executor::{
    scenario::callback_and_continue,
    types::{Handle, StreamRead, StreamSocket, StreamWrite},
    utils1::{HandleExt, HandleExt2, StreamSocketWithDropNotification, TaskHandleExt2},
};

use super::{scenario::ScenarioAccess, types::Task, utils1::RhResult};

//@ Connect to an intra-Websocat stream socket listening on specified virtual address.
fn connect_registry_stream(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_registry_stream");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Opts {
        addr: String,

        //@ Maximum size of buffer for data in flight
        max_buf_size: usize,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let tx = the_scenario.registry.get_sender(&opts.addr);

    debug!(parent: &span, addr=%opts.addr, "options parsed");

    let max_buf_size = opts.max_buf_size;

    drop(opts);

    Ok(async move {
        debug!("node started");

        let (r1, w1) = tokio::io::simplex(max_buf_size);
        let (r2, w2) = tokio::io::simplex(max_buf_size);

        let s1 = StreamSocket {
            read: Some(StreamRead {
                reader: Box::pin(r1),
                prefix: Default::default(),
            }),
            write: Some(StreamWrite {
                writer: Box::pin(w2),
            }),
            close: None,
            fd: None,
        };

        let s2 = StreamSocket {
            read: Some(StreamRead {
                reader: Box::pin(r2),
                prefix: Default::default(),
            }),
            write: Some(StreamWrite {
                writer: Box::pin(w1),
            }),
            close: None,
            fd: None,
        };

        let h2 = s2.wrap();

        match tx.send_async(rhai::Dynamic::from(h2)).await {
            Ok(()) => {}
            Err(e) => {
                error!("Failed to connect to a registry stream socket");
                return Err(e.into());
            }
        }
        
        debug!(s=?s1, "connected");

        let h1 = s1.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h1,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Listen for intra-Websocat stream socket connections on a specified virtual address
fn listen_registry_stream(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;
    let span = debug_span!("listen_registry_stream");
    debug!(parent: &span ,"node created");
    #[derive(serde::Deserialize)]
    struct Opts {
        addr: String,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let l = the_scenario.registry.get_receiver(&opts.addr);

    debug!(parent: &span, listen_addr=%opts.addr, "options parsed");

    let autospawn = opts.autospawn;
    let oneshot = opts.oneshot;
    drop(opts);

    Ok(async move {
        debug!("node started");
        let mut drop_nofity_r = None;
        let mut drop_nofity_w = None;

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.recv_async().await {
                Ok(d) => {
                    let newspan = debug_span!("registry_accept");

                    let Some(mut h) = d.try_cast::<Handle<StreamSocket>>() else {
                        error!("Something other than stream socket was sent to a listen_registry_stream: endpoint");
                        continue;
                    };

                    if oneshot {
                        let Some(mut s) = h.lut() else {
                            error!("Empty handle was sent to a listen_registry_stream: endpoint");
                            break;
                        };
                        if let Some(x) = s.read.take() {
                            let (sr,dnr) = StreamSocketWithDropNotification::wrap(x.reader);
                            drop_nofity_r = Some(dnr);
                            s.read = Some(StreamRead {
                                prefix: x.prefix,
                                reader: Box::pin(sr),
                            });
                        }
                        if let Some(x) = s.write.take() {
                            let (sw,dnw) = StreamSocketWithDropNotification::wrap(x.writer);
                            drop_nofity_w = Some(dnw);
                            s.write = Some(StreamWrite {
                                writer: Box::pin(sw),
                            });
                        }
                        debug!(parent: &newspan, ?s, "accepted");
                        h = Some(s).wrap();
                    } else {
                        debug!(parent: &newspan, "accepted");
                    }


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
                    return Err(e.into());
                }
            }
            if oneshot {
                debug!("Exiting registry listener due to --oneshot mode");
                break
            }
        }

        if let Some(dn) = drop_nofity_r {
            debug!("Waiting for the sole accepted client to finish serving reads");
            let _ = dn.await;
        }
        if let Some(dn) = drop_nofity_w {
            debug!("Waiting for the sole accepted client to finish serving writes");
            let _ = dn.await;
        }
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("listen_registry_stream", listen_registry_stream);
    engine.register_fn("connect_registry_stream", connect_registry_stream);
}
