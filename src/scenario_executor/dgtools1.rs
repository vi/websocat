use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Poll},
};

use futures::FutureExt;
use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::PollSemaphore;
use tracing::{debug, debug_span};

use crate::scenario_executor::{
    utils1::{ExtractHandleOrFail, SimpleErr},
    utils2::PollSemaphoreNew2,
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, Hangup,
        PacketRead, PacketReadResult, PacketWrite,
    },
    utils1::{HandleExt, RhResult},
};

pub struct SimpleReuser {
    inner: DatagramSocket,
    w_sem: PollSemaphore,
    r_sem: PollSemaphore,
    shared_close_notifier: Option<futures::future::Shared<Hangup>>,
}

struct SimpleReuserWriter {
    inner: Handle<SimpleReuser>,
    w_sem_permit: Option<OwnedSemaphorePermit>,
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
        if this.w_sem_permit.is_none() {
            match ready!(inner.w_sem.poll_acquire(cx)) {
                None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Some(p) => this.w_sem_permit = Some(p),
            }
        }

        let Some(ref mut w) = inner.inner.write else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        let ret = ready!(PacketWrite::poll_write(w.snk.as_mut(), cx, buf, flags));
        this.w_sem_permit = None;

        Poll::Ready(ret)
    }
}

struct SimpleReuserReader {
    inner: Handle<SimpleReuser>,
    r_sem_permit: Option<OwnedSemaphorePermit>,
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
        }

        let Some(ref mut r) = inner.inner.read else {
            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()));
        };

        let ret = ready!(PacketRead::poll_read(r.src.as_mut(), cx, buf,));

        if let Ok(ref ret) = ret {
            if ret.flags.contains(BufferFlag::NonFinalChunk) {
                // TODO
            }
        }

        this.r_sem_permit = None;

        Poll::Ready(ret)
    }
}

//@ Create object that multiplexes multiple DatagramSocket connections into one,
//@ forwarding inner reads to arbitrary outer readers.
//@
//@ If inner socket disconnects, reuser will not attempt to reestablish the connection
fn simple_reuser(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Datagram socket to multiplex connections to
    inner: Handle<DatagramSocket>,
) -> RhResult<Handle<SimpleReuser>> {
    let span = debug_span!("reuser");
    #[derive(serde::Deserialize)]
    struct Opts {}
    let mut inner = ctx.lutbar(inner)?;
    let _opts: Opts = rhai::serde::from_dynamic(&opts)?;

    debug!(parent: &span, "options parsed");

    let shared_close_notifier = inner.close.take().map(|x| x.shared());
    let reuser = SimpleReuser {
        inner,
        w_sem: PollSemaphore::new2(1),
        r_sem: PollSemaphore::new2(1),
        shared_close_notifier,
    };

    Ok(Some(reuser).wrap())
}

//@ Create object that multiplexes multiple DatagramSocket connections into one,
//@ forwarding inner reads to arbitrary outer readers
fn simple_reuser_connect(
    ctx: NativeCallContext,
    reuser: &mut Handle<SimpleReuser>,
) -> RhResult<Handle<DatagramSocket>> {
    let r1 = reuser.clone();
    let r2 = reuser.clone();
    let mut reuser = reuser.lock().unwrap();
    let Some(reuser) = reuser.as_mut() else {
        return Err(ctx.err("Null reuser handle"));
    };

    let r = SimpleReuserReader {
        inner: r1,
        r_sem_permit: None,
    };

    let w = SimpleReuserWriter {
        inner: r2,
        w_sem_permit: None,
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

pub fn register(engine: &mut Engine) {
    engine.register_fn("simple_reuser", simple_reuser);
    engine.register_fn("connect", simple_reuser_connect);
}
