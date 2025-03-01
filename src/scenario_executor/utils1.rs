use futures::Future;
use rhai::{EvalAltResult, NativeCallContext};
use tokio::io::{AsyncRead, AsyncWrite};
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

pub const NEUTRAL_SOCKADDR4: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::UNSPECIFIED,
    0,
));
pub const NEUTRAL_SOCKADDR6: SocketAddr = SocketAddr::V6(std::net::SocketAddrV6::new(
    std::net::Ipv6Addr::UNSPECIFIED,
    0,
    0,
    0,
));

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

pub struct SignalOnDrop(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for SignalOnDrop {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

impl SignalOnDrop {
    pub fn defuse(&mut self) {
        let _ = self.0.take();
    }
}

impl SignalOnDrop {
    pub fn new() -> (SignalOnDrop, tokio::sync::oneshot::Receiver<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (SignalOnDrop(Some(tx)), rx)
    }
    pub const fn new_neutral() -> SignalOnDrop {
        SignalOnDrop(None)
    }
}

#[pin_project::pin_project]
pub struct StreamSocketWithDropNotification<T> {
    #[pin]
    inner: T,
    dropper: SignalOnDrop,
}

impl<T> StreamSocketWithDropNotification<T> {
    pub fn wrap(inner: T) -> (Self, tokio::sync::oneshot::Receiver<()>) {
        let (dropper, rx) = SignalOnDrop::new();
        (StreamSocketWithDropNotification { inner, dropper }, rx)
    }

    pub fn defuse(self: Pin<&mut Self>) {
        let this = self.project();
        this.dropper.defuse();
    }
}

impl<T: AsyncRead> AsyncRead for StreamSocketWithDropNotification<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();
        this.inner.poll_read(cx, buf)
    }
}

impl<T: AsyncWrite> AsyncWrite for StreamSocketWithDropNotification<T> {
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.project();
        this.inner.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.project();
        this.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}

pub fn wrap_as_stream_socket<R: AsyncRead + Send + 'static, W: AsyncWrite + Send + 'static>(
    r: R,
    w: W,
    close: Option<Hangup>,
    needs_drop_monitor: bool,
) -> (
    StreamSocket,
    Option<(
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
) {
    if !needs_drop_monitor {
        let (r, w) = (Box::pin(r), Box::pin(w));

        (
            StreamSocket {
                read: Some(StreamRead {
                    reader: r,
                    prefix: Default::default(),
                }),
                write: Some(StreamWrite { writer: w }),
                close,
            },
            None,
        )
    } else {
        let (r, dn1) = StreamSocketWithDropNotification::wrap(r);
        let (w, dn2) = StreamSocketWithDropNotification::wrap(w);

        let (r, w) = (Box::pin(r), Box::pin(w));

        (
            StreamSocket {
                read: Some(StreamRead {
                    reader: r,
                    prefix: Default::default(),
                }),
                write: Some(StreamWrite { writer: w }),
                close: None,
            },
            Some((dn1, dn2)),
        )
    }
}
