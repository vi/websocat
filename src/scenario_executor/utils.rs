use futures::Future;
use rhai::{EvalAltResult, NativeCallContext};
use tokio::io::AsyncRead;
use tracing::{error, trace};

use crate::scenario_executor::types::{DatagramRead, DatagramWrite, Handle, StreamSocket, Task};
use std::{
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use super::types::{
    BufferFlag, BufferFlags, DatagramSocket, Hangup, PacketWrite, StreamRead, StreamWrite,
};

pub trait TaskHandleExt {
    fn wrap_noerr(self) -> Handle<Task>;
}
pub trait TaskHandleExt2 {
    fn wrap(self) -> Handle<Task>;
}
pub trait HangupHandleExt {
    fn wrap(self) -> Handle<Hangup>;
}

impl<T: Future<Output = ()> + Send + 'static> TaskHandleExt for T {
    fn wrap_noerr(self) -> Handle<Task> {
        use futures::FutureExt;
        Arc::new(Mutex::new(Some(Box::pin(self.map(|_| Ok(()))))))
    }
}
impl<T: Future<Output = anyhow::Result<()>> + Send + 'static> TaskHandleExt2 for T {
    fn wrap(self) -> Handle<Task> {
        Arc::new(Mutex::new(Some(Box::pin(self))))
    }
}
impl<T: Future<Output = ()> + Send + 'static> HangupHandleExt for T {
    fn wrap(self) -> Handle<Hangup> {
        Arc::new(Mutex::new(Some(Box::pin(self))))
    }
}

pub trait HandleExt {
    type HandleInner;
    fn wrap(self) -> Handle<Self::HandleInner>;
}

impl<T> HandleExt for Option<T> {
    type HandleInner = T;
    fn wrap(self) -> Handle<T> {
        Arc::new(Mutex::new(self))
    }
}

pub trait HandleExt2 {
    type Target;
    /// Lock, unwrap and take
    fn lut(&self) -> Self::Target;
}

impl<T> HandleExt2 for Handle<T> {
    type Target = Option<T>;
    fn lut(&self) -> Self::Target {
        self.lock().unwrap().take()
    }
}

pub async fn run_task(h: Handle<Task>) {
    let Some(t) = h.lock().unwrap().take() else {
        error!("Attempt to run a null/taken task");
        return;
    };
    if let Err(e) = t.await {
        error!("{e}");
    }
}

impl StreamSocket {
    pub fn wrap(self) -> Handle<StreamSocket> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramRead {
    pub fn wrap(self) -> Handle<DatagramRead> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramWrite {
    pub fn wrap(self) -> Handle<DatagramWrite> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl StreamRead {
    pub fn wrap(self) -> Handle<StreamRead> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl StreamWrite {
    pub fn wrap(self) -> Handle<StreamWrite> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramSocket {
    pub fn wrap(self) -> Handle<DatagramSocket> {
        Arc::new(Mutex::new(Some(self)))
    }
}

#[must_use]
pub struct PutItBack<T>(pub Handle<T>);

impl<T> PutItBack<T> {
    pub fn put(self, x: T) {
        *self.0.lock().unwrap() = Some(x)
    }
}

pub trait ExtractHandleOrFail {
    /// Lock mutex, Unwrapping possible poison error, Take the thing from option contained inside, fail if is is none and convert the error to BoxAltResult.
    fn lutbar<T>(&self, mut h: Handle<T>) -> Result<T, Box<EvalAltResult>> {
        self.lutbarm(&mut h)
    }
    fn lutbar2<T>(&self, h: Handle<T>) -> Result<(T, PutItBack<T>), Box<EvalAltResult>> {
        let hh = h.clone();
        Ok((self.lutbar(h)?, PutItBack(hh)))
    }
    fn lutbarm<T>(&self, h: &mut Handle<T>) -> Result<T, Box<EvalAltResult>>;
    fn lutbar2m<T>(&self, h: &mut Handle<T>) -> Result<(T, PutItBack<T>), Box<EvalAltResult>> {
        let hh = h.clone();
        Ok((self.lutbar(h.clone())?, PutItBack(hh)))
    }
}
impl ExtractHandleOrFail for NativeCallContext<'_> {
    fn lutbarm<T>(&self, h: &mut Handle<T>) -> Result<T, Box<EvalAltResult>> {
        match h.lut() {
            Some(x) => Ok(x),
            None => Err(self.err("Null handle")),
        }
    }
}

pub type RhResult<T> = Result<T, Box<EvalAltResult>>;

impl AsyncRead for StreamRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let sr = self.get_mut();

        if !sr.prefix.is_empty() {
            let limit = buf.remaining().min(sr.prefix.len());
            trace!(nbytes = limit, "Serving from prefix");
            buf.put_slice(&sr.prefix.split_to(limit));
            return Poll::Ready(Ok(()));
        }

        sr.reader.as_mut().poll_read(cx, buf)
    }
}

pub trait SimpleErr {
    fn err(&self, v: impl Into<rhai::Dynamic>) -> Box<EvalAltResult>;
}
impl SimpleErr for NativeCallContext<'_> {
    fn err(&self, v: impl Into<rhai::Dynamic>) -> Box<EvalAltResult> {
        Box::new(EvalAltResult::ErrorRuntime(v.into(), self.position()))
    }
}

pub struct DisplayBufferFlags(pub BufferFlags);

impl std::fmt::Display for DisplayBufferFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for x in self.0 {
            match x {
                BufferFlag::NonFinalChunk => f.write_str("C")?,
                BufferFlag::Text => f.write_str("T")?,
                BufferFlag::Eof => f.write_str("E")?,
                BufferFlag::Ping => f.write_str("P")?,
                BufferFlag::Pong => f.write_str("O")?,
            }
        }
        Ok(())
    }
}

pub trait ToNeutralAddress {
    /// Convert socket address to 0.0.0.0:0 (or `[::]:0`) based on AF_FAMILY of other socket address
    fn to_neutral_address(&self) -> Self;
}

impl ToNeutralAddress for SocketAddr {
    fn to_neutral_address(&self) -> Self {
        match self {
            SocketAddr::V4(_) => {
                SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0)
            }
            SocketAddr::V6(_) => {
                SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0)
            }
        }
    }
}

pub trait IsControlFrame {
    fn is_control(&self) -> bool;
}

impl IsControlFrame for BufferFlags {
    fn is_control(&self) -> bool {
        self.contains(BufferFlag::Eof)
            || self.contains(BufferFlag::Ping)
            || self.contains(BufferFlag::Pong)
    }
}

pub trait PacketWriteExt {
    fn send_eof(self) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

impl<T: PacketWrite + Send + ?Sized> PacketWriteExt for Pin<&mut T> {
    fn send_eof(mut self) -> impl std::future::Future<Output = std::io::Result<()>> + Send {
        std::future::poll_fn(move |cx| {
            let mut b = [];
            PacketWrite::poll_write(self.as_mut(), cx, &mut b, BufferFlag::Eof.into())
        })
    }
}



#[derive(Debug, Clone)]
#[pin_project::pin_project]
pub struct MyOptionFuture<F> {
    #[pin]
    inner: Option<F>,
}


impl<F> Default for MyOptionFuture<F> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<F: Future> Future for MyOptionFuture<F> {
    type Output = Option<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().inner.as_pin_mut() {
            Some(x) => x.poll(cx).map(Some),
            None => Poll::Ready(None),
        }
    }
}

impl<T> From<Option<T>> for MyOptionFuture<T> {
    fn from(option: Option<T>) -> Self {
        Self { inner: option }
    }
}

impl<T> MyOptionFuture<T> {
    pub fn take(&mut self) -> Option<T> {
        self.inner.take()
    }
}
