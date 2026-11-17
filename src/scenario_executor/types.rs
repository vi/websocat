use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use bytes::BytesMut;
use futures::Future;
use rhai::Dynamic;
use tokio::io::{AsyncRead, AsyncWrite};
pub type Handle<T> = Arc<Mutex<Option<T>>>;

pub type Task = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;
pub type Hangup = Pin<Box<dyn Future<Output = ()> + Send>>;

pub type DiagnosticOutput = Box<dyn std::io::Write + Send>;
pub type RandomnessSource = Box<dyn rand::RngCore + Send>;

pub struct StreamRead {
    pub reader: Pin<Box<dyn AsyncRead + Send>>,
    pub prefix: BytesMut,
}
pub struct StreamWrite {
    pub writer: Pin<Box<dyn AsyncWrite + Send>>,
}

/// File descriptor of the underlying socket, ignoring the overlays.
///
/// Note that `BorrowedFd` is used not according it its semantics, just to gain stable access to the niche at -1; like an optimized `RawFd`.
#[cfg(unix)]
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct SocketFd(pub std::os::fd::BorrowedFd<'static>);

#[cfg(not(unix))]
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct SocketFd(pub std::convert::Infallible);

pub struct StreamSocket {
    pub read: Option<StreamRead>,
    pub write: Option<StreamWrite>,
    pub close: Option<Hangup>,
    pub fd: Option<SocketFd>,
}

flagset::flags! {
    pub enum BufferFlag : u8 {
        /// This buffer denotes some incomplete chunk of a multi-chunk message.
        NonFinalChunk,
        /// When used in WebSocket context, this denotes this buffer relates to some text data, not binary.
        Text,
        /// End of stream, when used in [`PacketRead::poll_read`]
        Eof,
        /// This buffer corresponds to a WebSocket ping
        Ping,
        /// This buffer corresponds to a WebSocket pong
        Pong,
    }
}
pub type BufferFlags = flagset::FlagSet<BufferFlag>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketReadResult {
    pub flags: BufferFlags,
    pub buffer_subset: std::ops::Range<usize>,
}

/// Similar to `tokio::io::AsyncRead`, but for buffer boundaries are
/// significant and there additional flags beside each buffer.
///
/// Zero-length reads do not mean EOF.
///
/// Stream/Sink are not used instead to control the allocations.
///
/// When `poll_read` returns, subsequent `poll_read` can expect data in `buf`
/// outside of the range returned in `buffer_subset` to remain the same,
/// though buffer address in memory may be different.
/// Bytes referenced by `buffer_subset` data may be mangled, e.g. by `poll_write`
/// using mutable chunk buffer of the same buffer.
/// This should be honoured both when driving a read of a packet to complection (`Poll::Pending`)
/// and also between packets. Buffer shrinkage or data mangling
/// may result in a panic or malformed data.
/// 
/// It is OK to abandon a packet source by `Drop::drop`-ing it
/// while ` poll_read` was in progress (i.e. returned `Pending`).
pub trait PacketRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>>;
}

/// Similar to `tokio::io::AsyncWrite`, but for buffer boundaries are significant and there additional flags beside each buffer.
///
/// There are no partial writes or explicit flushes.
///
/// Stream/Sink are not used instead to control the allocations.
///
/// Writing (possibly empty) buffer with Eof flag means something like `poll_shutdown()`.
///
/// When `Poll::Pending` is returned, next call to `poll_write` should use the same arguments.
/// Sinks may panic or mangle the data if buffer suddenly shrinks or content unexpectadle changes.
/// Deliberate abrupt sink close should be done with `Drop::drop` without trying to push a EOF event.
///
/// Memory address of the buffer may be different, but content should be the same.
///
/// Buffer content may be modified by writer (for in-place transformation instead of allocations).
/// 
/// In future, [`PacketWrite::poll_write`]'s `buf` may become a scratch space buffer, with a new `buffer_subset`
/// argument similar to [`PacketReadResult::buffer_subset`] aid that idea and allow
/// overlays to prepend and append content to packets without allocations, fragmentation or using vectored writes.
/// (This may require some care in `CopyPackets`, as both packet sources and sinks may want scratch space 
/// and packet sources may expect their data to stay unmodified between packets - more design decisions may be needed)
pub trait PacketWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>>;
}

pub struct DatagramRead {
    pub src: Pin<Box<dyn PacketRead + Send>>,
}
pub struct DatagramWrite {
    pub snk: Pin<Box<dyn PacketWrite + Send>>,
}
pub struct DatagramSocket {
    pub read: Option<DatagramRead>,
    pub write: Option<DatagramWrite>,
    pub close: Option<Hangup>,
    pub fd: Option<SocketFd>,
}

pub type UniversalChannel = (flume::Sender<Dynamic>, flume::Receiver<Dynamic>);

#[derive(Debug, Clone, Default)]
pub struct Registry(pub(super) Arc<Mutex<std::collections::HashMap<String, UniversalChannel>>>);

pub type DatagramSocketSlot = tokio::sync::oneshot::Sender<DatagramSocket>;

pub type Slot = tokio::sync::oneshot::Sender<Dynamic>;
pub type Promise = tokio::sync::oneshot::Receiver<Dynamic>;
pub type ChannelSender = flume::Sender<Dynamic>;
pub type ChannelReceiver = flume::Receiver<Dynamic>;
