use std::{
    io::ErrorKind,
    pin::Pin,
    sync::Arc,
    task::{ready, Poll},
};

use futures::FutureExt;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::PollSemaphore;
use tracing::{debug, debug_span, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    utils1::{ExtractHandleOrFail, SimpleErr, TaskHandleExt2},
    utils2::PollSemaphoreNew2,
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramSocketSlot, DatagramWrite,
        Handle, Hangup, PacketRead, PacketReadResult, PacketWrite, Task,
    },
    utils1::{HandleExt, RhResult},
};

pub struct SimpleReuser {
    inner: DatagramSocket,
    w_sem: PollSemaphore,
    r_sem: PollSemaphore,
    shared_close_notifier: Option<futures::future::Shared<Hangup>>,
    disconnect_on_torn_datagram: bool,
    reading_message_in_progress: bool,
    writing_message_in_progress: Option<BufferFlags>,
    writing_closed: bool,
}

enum SimpleReuserListenerInner {
    Uninitialized,
    Active(Handle<SimpleReuser>),
    Failed,
}
// Note: Outer mutex in Handle<SimpleReuserListener> is extraneous and is just to avoid being different from other similar types
pub struct SimpleReuserListener(Arc<tokio::sync::Mutex<SimpleReuserListenerInner>>);

struct SimpleReuserWriter {
    inner: Handle<SimpleReuser>,
    w_sem_permit: Option<OwnedSemaphorePermit>,
    torn_message_measures: bool,
    closemsg: Option<Box<[u8; 97]>>,
}

impl PacketWrite for SimpleReuserWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        let mut inner = this.inner.lock().unwrap();
        let Some(inner) = inner.as_mut() else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        if inner.writing_closed {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        }

        if flags.contains(BufferFlag::Eof) {
            // do not propagate client disconnections to the reuser's inner socket
            debug!("reuser client's disconnect");
            return Poll::Ready(Ok(()));
        }

        if this.w_sem_permit.is_none() {
            match ready!(inner.w_sem.poll_acquire(cx)) {
                None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Some(p) => this.w_sem_permit = Some(p),
            }
            if inner.writing_message_in_progress.is_some() {
                // semaphore was released by other client without resetting
                // `writing_message_in_progress` back to `false`.
                //
                // This means other client abruptly disconnected
                // while writing a chunked message without finishing it
                // properly.

                if inner.disconnect_on_torn_datagram {
                    warn!("Shutting down writing to the reuser because of a broken message. Use --reuser-tolerate-torn-msgs flag to prefer mangled messages instead of disconnections.");
                    this.torn_message_measures = true;
                } else {
                    warn!("Abrupt client disconnection caused a mangled message to be delivered to reuser's inner socket");
                    this.torn_message_measures = true;
                    return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
                }
            }
        }

        let Some(ref mut w) = inner.inner.write else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        if this.torn_message_measures {
            if inner.disconnect_on_torn_datagram {
                if this.closemsg.is_none() {
                    this.closemsg = Some(Box::new(*b"Partially written message to Websocat's `reuse-raw:` prevents further messages in this connection"));
                }

                ready!(PacketWrite::poll_write(
                    w.snk.as_mut(),
                    cx,
                    &mut this.closemsg.as_mut().unwrap()[..],
                    BufferFlag::Eof.into()
                ))?;
                inner.writing_closed = true;
            } else {
                let mut flags = inner.writing_message_in_progress.unwrap();
                flags -= BufferFlag::NonFinalChunk;
                ready!(PacketWrite::poll_write(w.snk.as_mut(), cx, &mut [], flags))?;
            }
            inner.writing_message_in_progress = None;
            this.torn_message_measures = false;
            this.closemsg = None;
        }

        let ret = ready!(PacketWrite::poll_write(w.snk.as_mut(), cx, buf, flags));

        if flags.contains(BufferFlag::NonFinalChunk) {
            inner.writing_message_in_progress = Some(flags);
        } else {
            inner.writing_message_in_progress = None;
            this.w_sem_permit = None;
        }

        Poll::Ready(ret)
    }
}

struct SimpleReuserReader {
    inner: Handle<SimpleReuser>,
    r_sem_permit: Option<OwnedSemaphorePermit>,
    /// We have encountered a fragment of a message partially delivered
    /// to other client and need to discard the remaining fragments of it prior
    /// to reading the next message
    discard_this_message: bool,
}

impl PacketRead for SimpleReuserReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();

        let mut inner = this.inner.lock().unwrap();
        let Some(inner) = inner.as_mut() else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        if this.r_sem_permit.is_none() {
            match ready!(inner.r_sem.poll_acquire(cx)) {
                None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Some(p) => this.r_sem_permit = Some(p),
            }

            if inner.reading_message_in_progress {
                debug!("Discarding the remains of a half-delivered message");
                this.discard_this_message = true;
            }
        }

        let Some(ref mut r) = inner.inner.read else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        loop {
            let ret = ready!(PacketRead::poll_read(r.src.as_mut(), cx, buf,));

            if let Ok(ref ret) = ret {
                match (
                    this.discard_this_message,
                    ret.flags.contains(BufferFlag::NonFinalChunk),
                ) {
                    (true, false) => {
                        this.discard_this_message = false;
                        continue;
                    }
                    (true, true) => {
                        continue;
                    }
                    (false, true) => {
                        inner.reading_message_in_progress = true;
                        // do not release the semaphore permit yet
                        return Poll::Ready(Ok(ret.clone()));
                    }
                    (false, false) => {
                        inner.reading_message_in_progress = false;
                        this.r_sem_permit = None;

                        return Poll::Ready(Ok(ret.clone()));
                    }
                }
            }
        }
    }
}

//@ Create an inactive SimpleReuserListener.
//@ It becomes active when `maybe_init_then_connect` is called the first time
fn simple_reuser_listener() -> RhResult<Handle<SimpleReuserListener>> {
    Ok(Some(SimpleReuserListener(Arc::new(tokio::sync::Mutex::new(
        SimpleReuserListenerInner::Uninitialized,
    ))))
    .wrap())
}

fn simple_reuser_inner(
    mut inner: DatagramSocket,
    disconnect_on_torn_datagram: bool,
) -> Handle<SimpleReuser> {
    let shared_close_notifier = inner.close.take().map(|x| x.shared());
    let reuser = SimpleReuser {
        inner,
        w_sem: PollSemaphore::new2(1),
        r_sem: PollSemaphore::new2(1),
        shared_close_notifier,
        disconnect_on_torn_datagram,
        reading_message_in_progress: false,
        writing_message_in_progress: None,
        writing_closed: false,
    };

    Some(reuser).wrap()
}

//@ Create object that multiplexes multiple DatagramSocket connections into one,
//@ forwarding inner reads to arbitrary outer readers.
//@
//@ If inner socket disconnects, reuser will not attempt to reestablish the connection
fn simple_reuser(
    ctx: NativeCallContext,
    //@ Datagram socket to multiplex connections to
    inner: Handle<DatagramSocket>,
    //@ Drop inner connection when user begins writing a message, but leaves before finishing it,
    //@ leaving inner connection with incomplete message that cannot ever be completed.
    //@ If `false`, the reuser would commit the torn message and continue processing.
    disconnect_on_torn_datagram: bool,
) -> RhResult<Handle<SimpleReuser>> {
    let inner = ctx.lutbar(inner)?;
    Ok(simple_reuser_inner(inner, disconnect_on_torn_datagram))
}

fn simple_reuser_connect_inner<E>(
    reuser: &Handle<SimpleReuser>,
    on_null_handle: impl FnOnce() -> E,
) -> Result<Handle<DatagramSocket>, E> {
    let r1 = reuser.clone();
    let r2 = reuser.clone();
    let mut reuser = reuser.lock().unwrap();
    let Some(reuser) = reuser.as_mut() else {
        return Err(on_null_handle());
    };

    let r = SimpleReuserReader {
        inner: r1,
        r_sem_permit: None,
        discard_this_message: false,
    };

    let w = SimpleReuserWriter {
        inner: r2,
        w_sem_permit: None,
        torn_message_measures: false,
        closemsg: None,
    };

    let close = reuser
        .shared_close_notifier
        .clone()
        .map(|x| Box::pin(x) as Hangup);
    let s = DatagramSocket {
        read: Some(DatagramRead { src: Box::pin(r) }),
        write: Some(DatagramWrite { snk: Box::pin(w) }),
        close,
        fd: reuser.inner.fd,
    };

    debug!(s=?s, "reuser connect");

    Ok(Some(s).wrap())
}

//@ Obtain a shared DatagramSocket pointing to the socket that was specified as `inner` into `simple_reuser` function.
fn simple_reuser_connect(
    ctx: NativeCallContext,
    reuser: &mut Handle<SimpleReuser>,
) -> RhResult<Handle<DatagramSocket>> {
    simple_reuser_connect_inner(reuser, || ctx.err("Null reuser handle"))
}

//@ Initialize a persistent, shared DatagramSocket connection available for multiple clients (or just obtain a handle to it)
fn simple_reuser_listener_maybe_init_then_connect(
    ctx: NativeCallContext,
    reuser_l: &mut Handle<SimpleReuserListener>,
    opts: Dynamic,
    //@ Callback that is called on first call of this function and skipped on the rest (unless `recover` is set and needed)
    //@ The callback is supposed to send a DatagramSocket to the slot.
    initializer: FnPtr,
    //@ Callback that is called every time
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("reuser");
    let the_scenario = ctx.get_scenario()?;

    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Do not cache failed connection attempts, retry initialisation if a new client arrive.
        //@ Note that successful, but closed connections are not considered failed and that regard and will stay cached.
        //@ (use autoreconnect to handle that case)
        #[serde(default)]
        connect_again: bool,

        //@ Drop underlying connection if some client leaves in the middle of writing a message, leaving us with unrecoverably broken message.
        #[serde(default)]
        disconnect_on_broken_message: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "options parsed");

    let reuser_l = reuser_l.clone();
    Ok(async move {
        debug!("node started");

        let gg = {
            let reuser_g = reuser_l.lock().unwrap();
            if let Some(ref g) = *reuser_g {
                g.0.clone()
            } else {
                anyhow::bail!("Null reuser token")
            }
        };
        let mut gg = gg.lock().await;

        match *gg {
            SimpleReuserListenerInner::Failed if !opts.connect_again => {
                anyhow::bail!("This reuser previously failed initialisation");
            }
            SimpleReuserListenerInner::Active(ref mutex) => {
                debug!("reuser already initialised");

                let Ok(h) = simple_reuser_connect_inner(mutex, || ()) else {
                    anyhow::bail!("Empty reuser handle")
                };
                drop(gg);
                callback_and_continue::<(Handle<DatagramSocket>,)>(
                    the_scenario,
                    continuation,
                    (h,),
                )
                .await;
            }
            _ => {
                debug!("initializing reuser");

                let (tx, rx) = tokio::sync::oneshot::channel();

                let slot = Some(tx).wrap();
                let the_scenario_ = the_scenario.clone();
                callback_and_continue::<(Handle<DatagramSocketSlot>,)>(
                    the_scenario_,
                    initializer,
                    (slot,),
                )
                .await;

                debug!("returned from reuser's initializer");

                match rx.await {
                    Ok(s) => {
                        debug!("reuser initialisastion finished");

                        let rh = simple_reuser_inner(s, opts.disconnect_on_broken_message);
                        let rh2 = rh.clone();

                        *gg = SimpleReuserListenerInner::Active(rh2);

                        drop(gg);

                        let Ok(h) = simple_reuser_connect_inner(&rh, || ()) else {
                            anyhow::bail!("Empty reuser handle")
                        };

                        callback_and_continue::<(Handle<DatagramSocket>,)>(
                            the_scenario,
                            continuation,
                            (h,),
                        )
                        .await;
                    }
                    Err(_) => {
                        debug!("init failed");
                        *gg = SimpleReuserListenerInner::Failed;
                        anyhow::bail!("failed to initialize the reuser")
                    }
                }
            }
        }

        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Put DatagramSocket into its slot, e.g. to initialize a reuser.
//@
//@ Acts immediately and returns a dummy task just as a convenience (to make Rhai scripts typecheck).
fn dgslot_send(
    ctx: NativeCallContext,
    slot: &mut Handle<DatagramSocketSlot>,
    socket: Handle<DatagramSocket>,
) -> RhResult<Handle<Task>> {
    let so = ctx.lutbar(socket)?;
    let sl = ctx.lutbarm(slot)?;

    if sl.send(so).is_err() {
        return Err(ctx.err("Failed to fulfill a slot"));
    }

    Ok(super::trivials1::dummytask())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("simple_reuser", simple_reuser);
    engine.register_fn("connect", simple_reuser_connect);

    engine.register_fn("simple_reuser_listener", simple_reuser_listener);
    engine.register_fn(
        "maybe_init_then_connect",
        simple_reuser_listener_maybe_init_then_connect,
    );

    engine.register_fn("send", dgslot_send);
}
