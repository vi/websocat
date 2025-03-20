use std::time::Duration;

use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::io::AsyncWriteExt;
use tracing::{debug, debug_span, error, field, warn, Instrument};

use crate::scenario_executor::{
    debugfluff::PtrDbg,
    types::{DatagramRead, DatagramWrite, Handle, StreamRead, StreamSocket, StreamWrite, Task},
    utils1::{run_task, HandleExt, RhResult, TaskHandleExt},
};

use super::{
    http1::Http1Client,
    scenario::{callback_and_continue, ScenarioAccess},
    types::{DatagramSocket, Hangup},
    utils1::{ExtractHandleOrFail, HandleExt2, HangupHandleExt, SimpleErr, TaskHandleExt2},
    utils2::SocketFdI64,
};

//@ Modify stream-oriented Socket, taking the read part and returning it separately. Leaves behind an incomplete socket.
fn take_read_part(ctx: NativeCallContext, h: Handle<StreamSocket>) -> RhResult<Handle<StreamRead>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.read.take().wrap())
    } else {
        Err(ctx.err("StreamSocket is null"))
    }
}
//@ Modify stream-oriented Socket, taking the write part and returning it separately. Leaves behind an incomplete socket.
fn take_write_part(
    ctx: NativeCallContext,
    h: Handle<StreamSocket>,
) -> RhResult<Handle<StreamWrite>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.write.take().wrap())
    } else {
        Err(ctx.err("StreamSocket is null"))
    }
}
//@ Modify datagram-oriented Socket, taking the read part and returning it separately. Leaves behind an incomplete socket.
fn take_source_part(
    ctx: NativeCallContext,
    h: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramRead>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.read.take().wrap())
    } else {
        Err(ctx.err("StreamSocket is null"))
    }
}
//@ Modify datagram-oriented Socket, taking the write part and returning it separately. Leaves behind an incomplete socket.
fn take_sink_part(
    ctx: NativeCallContext,
    h: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramWrite>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.write.take().wrap())
    } else {
        Err(ctx.err("StreamSocket is null"))
    }
}
//@ Modify Socket, taking the hangup signal part, if it is set.
fn take_hangup_part(ctx: NativeCallContext, h: Dynamic) -> RhResult<Handle<Hangup>> {
    if let Some(h) = h.clone().try_cast::<Handle<StreamSocket>>() {
        return if let Some(s) = h.lock().unwrap().as_mut() {
            Ok(s.close.take().wrap())
        } else {
            Err(ctx.err("StreamSocket is null"))
        };
    }
    if let Some(h) = h.clone().try_cast::<Handle<DatagramSocket>>() {
        return if let Some(s) = h.lock().unwrap().as_mut() {
            Ok(s.close.take().wrap())
        } else {
            Err(ctx.err("DatagramSocket is null"))
        };
    }
    Err(ctx.err("take_hangup_part expects StreamSocket or DatagramSocket as argument"))
}

//@ Modify stream-oriented Socket, filling in the read direction with the specified one
fn put_read_part(
    ctx: NativeCallContext,
    h: Handle<StreamSocket>,
    x: Handle<StreamRead>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        s.read = x.lut();
        Ok(())
    } else {
        Err(ctx.err("StreamSocket null"))
    }
}

//@ Modify stream-oriented Socket, filling in the write direction with the specified one
fn put_write_part(
    ctx: NativeCallContext,
    h: Handle<StreamSocket>,
    x: Handle<StreamWrite>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        s.write = x.lut();
        Ok(())
    } else {
        Err(ctx.err("StreamSocket null"))
    }
}

//@ Modify datagram-oriented Socket, filling in the read direction with the specified one
fn put_source_part(
    ctx: NativeCallContext,
    h: Handle<DatagramSocket>,
    x: Handle<DatagramRead>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        s.read = x.lut();
        Ok(())
    } else {
        Err(ctx.err("DatagramSocket null"))
    }
}
//@ Modify datagram-oriented Socket, filling in the write direction with the specified one
fn put_sink_part(
    ctx: NativeCallContext,
    h: Handle<DatagramSocket>,
    x: Handle<DatagramWrite>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        s.write = x.lut();
        Ok(())
    } else {
        Err(ctx.err("DatagramSocket null"))
    }
}
//@ Modify Socket, filling in the hangup signal with the specified one
fn put_hangup_part(ctx: NativeCallContext, h: Dynamic, x: Handle<Hangup>) -> RhResult<()> {
    if let Some(h) = h.clone().try_cast::<Handle<StreamSocket>>() {
        return if let Some(s) = h.lock().unwrap().as_mut() {
            s.close = x.lut();
            Ok(())
        } else {
            Err(ctx.err("StreamSocket is null"))
        };
    }
    if let Some(h) = h.clone().try_cast::<Handle<DatagramSocket>>() {
        return if let Some(s) = h.lock().unwrap().as_mut() {
            s.close = x.lut();
            Ok(())
        } else {
            Err(ctx.err("DatagramSocket is null"))
        };
    }
    Err(ctx.err("take_hangup_part expects StreamSocket or DatagramSocket as argument"))
}

//@ A task that immediately finishes
pub fn dummytask() -> Handle<Task> {
    async move {}.wrap_noerr()
}

//@ A task that finishes after specified number of milliseconds
fn sleep_ms(ms: i64) -> Handle<Task> {
    async move {
        tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
        debug!("sleep_ms finished");
    }
    .wrap_noerr()
}

//@ Execute specified tasks in order, starting another and previous one finishes.
fn sequential(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        for t in tasks {
            if t.is_unit() {
                debug!("Ignoring null in sequential task list");
            } else if let Some(t) = t.clone().try_cast::<Handle<Task>>() {
                run_task(t).await;
            } else if let Some(h) = t.try_cast::<Handle<Hangup>>() {
                let Some(t) = h.lock().unwrap().take() else {
                    error!("Attempt to run a null/taken hangup handle");
                    continue;
                };
                t.await;
            } else {
                error!("Not a task or hangup in a list of tasks");
                continue;
            }
        }
    }
    .wrap_noerr()
}

//@ Execute specified tasks in parallel, waiting them all to finish.
fn parallel(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        let mut waitees = Vec::with_capacity(tasks.len());
        for t in tasks {
            let Some(t): Option<Handle<Task>> = t.try_cast() else {
                error!("Not a task in a list of tasks");
                continue;
            };
            waitees.push(tokio::spawn(run_task(t)));
        }
        for w in waitees {
            let _ = w.await;
        }
    }
    .wrap_noerr()
}

//@ Execute specified tasks in parallel, aborting all others if one of them finishes.
fn race(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        let mut waitees = Vec::with_capacity(tasks.len());
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        for t in tasks {
            let tx = tx.clone();
            if t.is_unit() {
                debug!("Ignoring null in sequential task list");
            } else if let Some(t) = t.clone().try_cast::<Handle<Task>>() {
                waitees.push(tokio::spawn(async move {
                    run_task(t).await;
                    let _ = tx.send(()).await;
                }));
            } else if let Some(h) = t.try_cast::<Handle<Hangup>>() {
                let Some(t) = h.lock().unwrap().take() else {
                    error!("Attempt to run a null/taken hangup handle");
                    continue;
                };
                waitees.push(tokio::spawn(async move {
                    t.await;
                    let _ = tx.send(()).await;
                }));
            } else {
                error!("Not a task or hangup in a list of tasks");
                continue;
            }
        }

        let _ = rx.recv().await;
        debug!("one of `race`'s task finished, aborting others");

        for w in waitees {
            w.abort();
        }
    }
    .wrap_noerr()
}

//@ Start execution of the specified task in background
fn spawn_task(task: Handle<Task>) {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "spawn", t = field::Empty);
    if let Some(x) = task.lock().unwrap().as_ref() {
        span.record("t", tracing::field::debug(PtrDbg(&**x)));
        debug!(parent: &span, "Spawning");
    } else {
        error!("Attempt to spawn a null/taken task");
    }
    tokio::spawn(
        async move {
            run_task(task).await;
            debug!("Finished");
        }
        .instrument(span),
    );
}

//@ Create null Hangup handle
fn empty_close_handle() -> Handle<Hangup> {
    None.wrap()
}

//@ Create a Hangup handle that immediately resolves (i.e. signals hangup)
fn pre_triggered_hangup_handle() -> Handle<Hangup> {
    use super::utils1::HangupHandleExt;
    async move {}.wrap()
}

//@ Create a Hangup handle that resolves after specific number of milliseconds
fn timeout_ms_hangup_handle(ms: i64) -> Handle<Hangup> {
    use super::utils1::HangupHandleExt;
    async move { tokio::time::sleep(Duration::from_millis(ms as u64)).await }.wrap()
}

//@ Exit Websocat process. If WebSocket is serving multiple connections, they all get aborted.
fn exit_process(code: i64) {
    debug!(code, "exit_process");
    std::process::exit(code as i32)
}

//@ Spawn a task that calls `continuation` when specified socket hangup handle fires
fn handle_hangup(
    ctx: NativeCallContext,
    hangup: Handle<Hangup>,
    continuation: FnPtr,
) -> RhResult<()> {
    let the_scenario = ctx.get_scenario()?;
    let hh = ctx.lutbar(hangup)?;
    tokio::spawn(async move {
        hh.await;
        debug!("handle_hangup");
        callback_and_continue::<()>(the_scenario, continuation, ()).await;
    });
    Ok(())
}

//@ Create hangup handle that gets triggered when specified task finishes.
fn task2hangup(
    ctx: NativeCallContext,
    task: Handle<Task>,
    //@ 0 means unconditionally, 1 means only when task has failed, 2 means only when task has succeeded.
    mode: i64,
) -> RhResult<Handle<Hangup>> {
    let x = ctx.lutbar(task)?;
    if !(0..=2).contains(&mode) {
        return Err(ctx.err("Invalid mode"));
    }
    let y = async move {
        let do_hangup = match (x.await, mode) {
            (Ok(()), 0 | 2) => {
                debug!("task completed, triggering the hangup handle");
                true
            }
            (Err(e), 0 | 1) => {
                debug!("task errored ({e}), triggering the hangup handle");
                true
            }
            (Ok(()), _) => false,
            (Err(e), _) => {
                debug!("task errored ({e})");
                false
            }
        };
        if !do_hangup {
            debug!("Locking this hangup handle infinitely");
            futures::future::pending().await
        }
    }
    .wrap();
    Ok(y)
}
//@ Convert a hangup token into a task. I don't know why this may be needed.
fn hangup2task(ctx: NativeCallContext, hangup: Handle<Hangup>) -> RhResult<Handle<Task>> {
    let x = ctx.lutbar(hangup)?;
    let y = async move {
        x.await;
        Ok(())
    }
    .wrap();
    Ok(y)
}

//@ Attempt to drop a socket or task or other handle
fn drop_thing(ctx: NativeCallContext, x: Dynamic) -> RhResult<()> {
    if let Some(t) = x.clone().try_cast::<Handle<Task>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a task");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<Hangup>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a hangup handle");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<StreamRead>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a stream reader");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<StreamWrite>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a stream writer");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<StreamSocket>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a stream socket");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramRead>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a datagram reader");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramWrite>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a datagram writer");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramSocket>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a datagram socket");
        drop(t)
    } else if let Some(t) = x.clone().try_cast::<Handle<Http1Client>>() {
        let t = ctx.lutbar(t)?;
        debug!("Explicitly dropping a http1 client");
        drop(t)
    } else {
        warn!("Trying to explicitly drop an unknown thing");
    }
    Ok(())
}

//@ Shutdown the writing part of a socket and drop it. If reading part was used extracted and used elswere, it stays active.
fn shutdown_and_drop(ctx: NativeCallContext, x: Dynamic) -> RhResult<()> {
    use super::utils1::PacketWriteExt;
    if let Some(t) = x.clone().try_cast::<Handle<StreamWrite>>() {
        let mut t = ctx.lutbar(t)?;
        debug!("Shuttind down and dropping a stream writer");
        tokio::spawn(async move {
            match t.writer.shutdown().await {
                Ok(()) => debug!("shutdown complete"),
                Err(e) => warn!("failed to shutdown a socket: {e}"),
            }
        });
    } else if let Some(t) = x.clone().try_cast::<Handle<StreamSocket>>() {
        let t = ctx.lutbar(t)?;
        debug!("Shuttind down and dropping a stream socket");
        if let Some(mut t) = t.write {
            tokio::spawn(async move {
                match t.writer.shutdown().await {
                    Ok(()) => debug!("shutdown complete"),
                    Err(e) => warn!("failed to shutdown a socket: {e}"),
                }
            });
        }
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramWrite>>() {
        let t = ctx.lutbar(t)?;
        debug!("Shuttind down and dropping a datagram writer");
        tokio::spawn(async move {
            match t.snk.send_eof().await {
                Ok(()) => debug!("shutdown complete"),
                Err(e) => warn!("failed to shutdown a socket: {e}"),
            }
        });
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramSocket>>() {
        let t = ctx.lutbar(t)?;
        debug!("Shuttind down and dropping a datagram socket");
        if let Some(t) = t.write {
            tokio::spawn(async move {
                match t.snk.send_eof().await {
                    Ok(()) => debug!("shutdown complete"),
                    Err(e) => warn!("failed to shutdown a socket: {e}"),
                }
            });
        }
    } else {
        return Err(ctx.err("shutdown_and_drop supports only sockets and writers"));
    }
    Ok(())
}

//@ Get underlying file descriptor from a socket, or -1 if is cannot be obtained
fn get_fd(ctx: NativeCallContext, x: Dynamic) -> RhResult<i64> {
    if let Some(t) = x.clone().try_cast::<Handle<StreamSocket>>() {
        let (t, b) = ctx.lutbar2(t)?;
        let fd = t.fd.maybe_as_i64();
        b.put(t);
        Ok(fd)
    } else if let Some(t) = x.clone().try_cast::<Handle<DatagramSocket>>() {
        let (t, b) = ctx.lutbar2(t)?;
        let fd = t.fd.maybe_as_i64();
        b.put(t);
        Ok(fd)
    } else if let Some(t) = x.clone().try_cast::<Handle<Http1Client>>() {
        let (t, b) = ctx.lutbar2(t)?;
        let fd = t.fd.maybe_as_i64();
        b.put(t);
        Ok(fd)
    } else {
        Err(ctx.err("Wrong object type to try get_fd"))
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("take_source_part", take_source_part);
    engine.register_fn("take_sink_part", take_sink_part);
    engine.register_fn("take_hangup_part", take_hangup_part);

    engine.register_fn("put_read_part", put_read_part);
    engine.register_fn("put_write_part", put_write_part);
    engine.register_fn("put_source_part", put_source_part);
    engine.register_fn("put_sink_part", put_sink_part);
    engine.register_fn("put_hangup_part", put_hangup_part);

    engine.register_fn("dummy_task", dummytask);
    engine.register_fn("sleep_ms", sleep_ms);
    engine.register_fn("sequential", sequential);
    engine.register_fn("parallel", parallel);
    engine.register_fn("race", race);
    engine.register_fn("spawn_task", spawn_task);

    engine.register_fn("empty_hangup_handle", empty_close_handle);
    engine.register_fn("pre_triggered_hangup_handle", pre_triggered_hangup_handle);
    engine.register_fn("timeout_ms_hangup_handle", timeout_ms_hangup_handle);

    engine.register_fn("exit_process", exit_process);
    engine.register_fn("handle_hangup", handle_hangup);

    engine.register_fn("task2hangup", task2hangup);
    engine.register_fn("hangup2task", hangup2task);

    engine.register_fn("drop", drop_thing);
    engine.register_fn("shutdown_and_drop", shutdown_and_drop);
    engine.register_fn("get_fd", get_fd);
}
