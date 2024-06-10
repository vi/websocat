use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use bytes::BytesMut;
use futures::Future;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
pub type Handle<T> = Arc<Mutex<Option<T>>>;

pub type Task = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type Hangup = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct StreamRead {
    pub reader: Pin<Box<dyn AsyncRead + Send>>,
    pub prefix: BytesMut,
}
pub struct StreamWrite {
    pub writer: Pin<Box<dyn AsyncWrite + Send>>,
}

pub struct StreamSocket {
    pub read: Option<StreamRead>,
    pub write: Option<StreamWrite>,
    pub close: Option<Hangup>,
}

flagset::flags! {
    pub enum BufferFlag : u8 {
        /// This buffer denotes some incomplete chunk of a multi-chunk message.
        NonFinalChunk,
        /// When used in WebSocket context, this denotes this buffer relates to some text data, not binary.
        Text,
        /// End of stream, when used in [`PacketRead::poll_read`]
        Eof,
    }
}
pub type BufferFlags = flagset::FlagSet<BufferFlag>;

/// Similar to `tokio::io::AsyncRead`, but for buffer boundaries are significant and there additional flags beside each buffer.
///
/// Zero-length reads do not mean EOF.
///
/// Stream/Sink are not used instead to control the allocations.
pub trait PacketRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<BufferFlags>>;
}

/// Similar to `tokio::io::AsyncWrite`, but for buffer boundaries are significant and there additional flags beside each buffer.
///
/// There are no partial writes or explicit flushes.
///
/// Stream/Sink are not used instead to control the allocations.
///
/// Writing (possibly empty) buffer with Eof flag means something like `poll_shutdown()`.
///
/// Implementer is supposed to use the `buf.filled()` part as a message to deliver.
///
/// When `Poll::Pending` is returned, next call to `poll_write` should use the same arguments.
///
/// Memory address of the buffer may be different, but content should be the same.
///
/// The unused space in the buffer may be used to store temporary content to drive one `poll_write` to completion.
pub trait PacketWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>>;
}

pub struct DatagramRead {
    pub src: Pin<Box<dyn PacketRead + Send>>,
}
pub struct DatagramWrite {
    pub snk: Pin<Box<dyn PacketWrite + Send>>,
}
