use std::{
    net::SocketAddr,
    task::{ready, Poll},
};

use futures::FutureExt;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, warn, Instrument};

use crate::scenario_executor::{
    scenario::callback_and_continue,
    types::{BufferFlag, Handle, Slot},
    utils1::HandleExt,
};

use super::{
    scenario::ScenarioAccess,
    types::{DatagramSocket, DatagramWrite, Hangup, PacketWrite, Promise, StreamSocket, Task},
    utils1::{ExtractHandleOrFail, RhResult, SimpleErr},
    utils2::{Defragmenter, DefragmenterAddChunkResult},
};

pub struct TriggerableEventTrigger {
    tx: tokio::sync::oneshot::Sender<()>,
}

pub struct TriggerableEvent {
    waiter_part: Option<Hangup>,
    trigger_part: Option<TriggerableEventTrigger>,
}

//@ Create new one-time synchronisation object that allows to trigger a hangup event explicitly from Rhai code.
fn triggerable_event_create() -> Handle<TriggerableEvent> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let signal = TriggerableEvent {
        waiter_part: Some(Box::pin(async move {
            let _ = rx.await;
        })),
        trigger_part: Some(TriggerableEventTrigger { tx }),
    };
    Some(signal).wrap()
}

//@ Take the waitable part (Hangup) from an object created by `triggerable_event_create`
fn triggerable_event_take_hangup(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEvent>,
) -> RhResult<Handle<Hangup>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.waiter_part.take().wrap())
    } else {
        Err(ctx.err("TriggerableEvent's hangup part is already taken"))
    }
}

//@ Take the activatable part from an object created by `triggerable_event_create`
fn triggerable_event_take_trigger(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEvent>,
) -> RhResult<Handle<TriggerableEventTrigger>> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        Ok(s.trigger_part.take().wrap())
    } else {
        Err(ctx.err("TriggerableEvent's trigger part is already taken"))
    }
}

//@ Trigger the activatable part from an object created by `triggerable_event_create`.
//@ This should cause a hangup even on the associated Hangup object.
fn triggerable_event_fire(
    ctx: NativeCallContext,
    h: &mut Handle<TriggerableEventTrigger>,
) -> RhResult<()> {
    if let Some(s) = h.lock().unwrap().take() {
        let _ = s.tx.send(());
        Ok(())
    } else {
        Err(ctx.err("TriggerableEventTrigger is already used"))
    }
}

//@ Create a Task that runs specified Rhai code when scheduled.
fn task_wrap(ctx: NativeCallContext, continuation: FnPtr) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;

    let t: Task = Box::pin(async move {
        debug!("task_wrap");
        callback_and_continue::<()>(the_scenario, continuation, ()).await;
        Ok(())
    });
    Ok(Some(t).wrap())
}

//@ Extract IP address from SocketAddr
fn sockaddr_get_ip(sa: &mut SocketAddr) -> String {
    format!("{}", sa.ip())
}

//@ Extract port from SocketAddr
fn sockaddr_get_port(sa: &mut SocketAddr) -> i64 {
    sa.port().into()
}

//@ Build SocketAddr from IP and port
fn make_socket_addr(ctx: NativeCallContext, ip: &str, port: i64) -> RhResult<SocketAddr> {
    if let Ok(ip) = ip.parse() {
        Ok(SocketAddr::new(ip, port as u16))
    } else {
        Err(ctx.err("Failed to parse IP address"))
    }
}

//@ Send some object to named slot in the registry.
//@ Blocks if no receivers yet.
fn registry_send(ctx: NativeCallContext, addr: &str, x: Dynamic) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;

    let span = debug_span!("registry_send",%addr);

    let tx = the_scenario.registry.get_sender(addr);

    let t: Task = Box::pin(
        async move {
            debug!("send");
            match tx.send_async(x).await {
                Ok(()) => {
                    debug!("sent");
                }
                Err(_) => {
                    debug!("failed");
                    anyhow::bail!("Failed registry_send");
                }
            }
            Ok(())
        }
        .instrument(span),
    );
    Ok(Some(t).wrap())
}

//@ Receive one object from a named slot in the registry and call `continuation` once for it
fn registry_recv_one(
    ctx: NativeCallContext,
    addr: &str,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;

    let span = debug_span!("registry_recv_one",%addr);

    let rx = the_scenario.registry.get_receiver(addr);

    let t: Task = Box::pin(
        async move {
            debug!("recv");
            match rx.recv_async().await {
                Ok(x) => {
                    debug!("received");
                    callback_and_continue::<(Dynamic,)>(the_scenario, continuation, (x,)).await;
                }
                Err(_) => {
                    debug!("failed");
                    anyhow::bail!("Failed registry_recv_one");
                }
            }
            Ok(())
        }
        .instrument(span),
    );
    Ok(Some(t).wrap())
}

//@ Receive all objects from a named slot in the registry and call `continuation` for each one
fn registry_recv_all(
    ctx: NativeCallContext,
    addr: &str,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;

    let span = debug_span!("registry_recv_all",%addr);

    let rx = the_scenario.registry.get_receiver(addr);

    let t: Task = Box::pin(
        async move {
            loop {
                let the_scenario = the_scenario.clone();
                let continuation = continuation.clone();
                debug!("recv");
                match rx.recv_async().await {
                    Ok(x) => {
                        debug!("received");
                        callback_and_continue::<(Dynamic,)>(the_scenario, continuation, (x,)).await;
                    }
                    Err(_) => {
                        debug!("failed");
                        anyhow::bail!("Failed registry_recv_all");
                    }
                }
            }
        }
        .instrument(span),
    );
    Ok(Some(t).wrap())
}

//@ Initialize multiple things in parallel using a array of closures, then call final closure with results of the initialisation
fn init_in_parallel(
    ctx: NativeCallContext,
    //@ Array of functions to call to prepare the `Vec<Dynamic>` for `continuation` below. Each function should have signature like `Fn(Slot) -> Task`.
    initialisers: Vec<Dynamic>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let the_scenario = ctx.get_scenario()?;

    let span = debug_span!("init_in_parallel");

    let mut receivers: Vec<Promise> = Vec::with_capacity(initialisers.len());
    let mut results: Vec<Dynamic> = Vec::with_capacity(initialisers.len());
    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(initialisers.len());

    for (i, initialiser) in initialisers.into_iter().enumerate() {
        let Some(initialiser): Option<FnPtr> = initialiser.try_cast() else {
            return Err(ctx.err("Non-closure element in array"));
        };

        let the_scenario = the_scenario.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        receivers.push(rx);

        let span = debug_span!(parent: &span, "initialiser", i);

        handles.push(tokio::spawn(
            async move {
                debug!("started");
                let tx: Handle<Slot> = Some(tx).wrap();
                callback_and_continue::<(Handle<Slot>,)>(the_scenario, initialiser, (tx,)).await;
            }
            .instrument(span),
        ));
    }
    debug!("started all initialisers");

    let t: Task = Box::pin(
        async move {
            for (i, rx) in receivers.into_iter().enumerate() {
                match rx.await {
                    Ok(x) => {
                        debug!(i, "received");
                        results.push(x);
                    }
                    Err(_) => {
                        debug!(i, "failed, cleaning up");
                        for h in handles {
                            h.abort();
                        }
                        anyhow::bail!("One of init_in_parallel's initialisers failed")
                    }
                }
            }

            callback_and_continue::<(Vec<Dynamic>,)>(the_scenario, continuation, (results,)).await;

            Ok(())
        }
        .instrument(span),
    );
    Ok(Some(t).wrap())
}

//@ Fulfill a Slot with a value, e.g to complete one of initialisers for `init_in_parallel`.
//@
//@ Acts immediately and returns a dummy task just as a convenience (to make Rhai scripts typecheck).
fn slot_send(
    ctx: NativeCallContext,
    slot: &mut Handle<Slot>,
    x: Dynamic,
) -> RhResult<Handle<Task>> {
    let sl = ctx.lutbarm(slot)?;

    if sl.send(x).is_err() {
        return Err(ctx.err("Failed to fulfill a slot"));
    }

    Ok(super::trivials1::dummytask())
}

//@ Take reading part s1 and write part of s2 and make a new socket that routes reads to s1 and writes to s2.
//@
//@ Close notifications (hangup tokens) are combined from s1 and s2.
//@
//@ Write part of s1 and read part of s2 remain in the original sockets which become incomplete (but not null).
fn combine_read_and_write_bytestream(
    ctx: NativeCallContext,
    s1: Handle<StreamSocket>,
    s2: Handle<StreamSocket>,
) -> RhResult<Handle<StreamSocket>> {
    let mut s1 = s1.lock().unwrap();
    let mut s2 = s2.lock().unwrap();

    let (Some(s1), Some(s2)) = (s1.as_mut(), s2.as_mut()) else {
        return Err(ctx.err("Null socket handle"));
    };

    let close: Option<Hangup> = match (s1.close.take(), s2.close.take()) {
        (None, None) => None,
        (None, Some(c)) => Some(c),
        (Some(c), None) => Some(c),
        (Some(a), Some(b)) => Some(Box::pin(futures::future::select(a, b).map(|_| ()))),
    };

    let s = StreamSocket {
        read: s1.read.take(),
        write: s2.write.take(),
        close,
        fd: None,
    };
    debug!(?s, "combine_read_and_write_bytestream");
    Ok(Some(s).wrap())
}

//@ Take reading part s1 and write part of s2 and make a new socket that routes reads to s1 and writes to s2.
//@
//@ Close notifications (hangup tokens) are combined from s1 and s2.
//@
//@ Write part of s1 and read part of s2 remain in the original sockets which become incomplete (but not null).
fn combine_read_and_write_datagram(
    ctx: NativeCallContext,
    s1: Handle<DatagramSocket>,
    s2: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let mut s1 = s1.lock().unwrap();
    let mut s2 = s2.lock().unwrap();

    let (Some(s1), Some(s2)) = (s1.as_mut(), s2.as_mut()) else {
        return Err(ctx.err("Null socket handle"));
    };

    let close: Option<Hangup> = match (s1.close.take(), s2.close.take()) {
        (None, None) => None,
        (None, Some(c)) => Some(c),
        (Some(c), None) => Some(c),
        (Some(a), Some(b)) => Some(Box::pin(futures::future::select(a, b).map(|_| ()))),
    };

    let s = DatagramSocket {
        read: s1.read.take(),
        write: s2.write.take(),
        close,
        fd: None,
    };
    debug!(?s, "combine_read_and_write_bytestream");
    Ok(Some(s).wrap())
}

struct DefragmentWrites {
    inner: DatagramWrite,
    defragmenter: Defragmenter,
}

impl PacketWrite for DefragmentWrites {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: super::types::BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        match this.defragmenter.add_chunk(buf, flags) {
            DefragmenterAddChunkResult::DontSendYet => return Poll::Ready(Ok(())),
            DefragmenterAddChunkResult::Continunous(newbuf) => {
                let ret = ready!(this.inner.snk.as_mut().poll_write(
                    cx,
                    newbuf,
                    flags - BufferFlag::NonFinalChunk
                ));
                this.defragmenter.clear();
                return Poll::Ready(ret);
            }
            DefragmenterAddChunkResult::SizeLimitExceeded(_) => {
                warn!("Too large datagram");
                return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
            }
        }
    }
}

const fn default_max_send_datagram_size() -> usize {
    655360
}

//@ Buffer up fragmets of messages written to this overlay and only issue complete writes to inner socket.
fn defragment_writes(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Defragmenter buffer limit
        #[serde(default = "default_max_send_datagram_size")]
        max_send_datagram_size: usize,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    let mut s = ctx.lutbar(inner)?;

    if let Some(w) = s.write.take() {
        s.write = Some(DatagramWrite {
            snk: Box::pin(DefragmentWrites {
                inner: w,
                defragmenter: Defragmenter::new(opts.max_send_datagram_size),
            }),
        });
    } else {
        warn!("defragment_writers is used on an incomplete socket without a writing part");
    }
    debug!(?s, "defragment_writes");
    let h = Some(s).wrap();
    Ok(h)
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("triggerable_event_create", triggerable_event_create);
    engine.register_fn("take_hangup", triggerable_event_take_hangup);
    engine.register_fn("take_trigger", triggerable_event_take_trigger);
    engine.register_fn("fire", triggerable_event_fire);
    engine.register_fn("task_wrap", task_wrap);
    engine.register_fn("get_ip", sockaddr_get_ip);
    engine.register_fn("get_port", sockaddr_get_port);
    engine.register_fn("make_socket_addr", make_socket_addr);
    engine.register_fn("registry_send", registry_send);
    engine.register_fn("registry_recv_one", registry_recv_one);
    engine.register_fn("registry_recv_all", registry_recv_all);
    engine.register_fn("init_in_parallel", init_in_parallel);
    engine.register_fn("send", slot_send);
    engine.register_fn(
        "combine_read_and_write_bytestream",
        combine_read_and_write_bytestream,
    );
    engine.register_fn(
        "combine_read_and_write_datagram",
        combine_read_and_write_datagram,
    );
    engine.register_fn("defragment_writes", defragment_writes);
}
