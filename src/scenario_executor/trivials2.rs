use std::{pin::Pin, task::Poll};

use bytes::BytesMut;
use pin_project::pin_project;
use rhai::{Engine, NativeCallContext};
use tokio::io::{AsyncRead, ReadBuf};
use tracing::debug;

use crate::scenario_executor::{
    types::{Handle, StreamRead},
    utils::{ExtractHandleOrFail, RhResult},
};

use super::{
    types::{BufferFlag, DatagramRead, DatagramSocket, DatagramWrite, PacketRead, PacketReadResult, PacketWrite, StreamSocket, StreamWrite},
    utils::HandleExt,
};

#[pin_project]
struct ReadChunkLimiter {
    #[pin]
    inner: StreamRead,
    limit: usize,
}

impl AsyncRead for ReadChunkLimiter {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();

        let b = buf.initialized_mut();
        let limit = b.len().min(*this.limit);
        let b = &mut b[0..limit];
        let mut rb = ReadBuf::new(b);

        match tokio::io::AsyncRead::poll_read(this.inner, cx, &mut rb) {
            Poll::Ready(Ok(())) => {
                let read_len = rb.filled().len();
                buf.advance(read_len);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn read_chunk_limiter(
    ctx: NativeCallContext,
    x: Handle<StreamRead>,
    limit: i64,
) -> RhResult<Handle<StreamRead>> {
    let x = ctx.lutbar(x)?;
    debug!(inner=?x, "read_chunk_limiter");
    let x = StreamRead {
        reader: Box::pin(ReadChunkLimiter {
            inner: x,
            limit: limit as usize,
        }),
        prefix: BytesMut::new(),
    };
    debug!(wrapped=?x, "read_chunk_limiter");
    Ok(x.wrap())
}

struct CacheBeforeStartingReading {
    inner: StreamRead,
    limit: usize,
}

impl AsyncRead for CacheBeforeStartingReading {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let sr: &mut StreamRead = &mut this.inner;

        if this.limit == 0 {}

        if !sr.prefix.is_empty() {
            let limit = buf.remaining().min(sr.prefix.len()).min(this.limit);
            buf.put_slice(&sr.prefix.split_to(limit));
            return Poll::Ready(Ok(()));
        }

        let b = buf.initialized_mut();
        let limit = b.len().min(this.limit);
        let b = &mut b[0..limit];
        let mut rb = ReadBuf::new(b);

        match tokio::io::AsyncRead::poll_read(sr.reader.as_mut(), cx, &mut rb) {
            Poll::Ready(Ok(())) => {
                let read_len = rb.filled().len();
                buf.advance(read_len);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

//@ Create stream socket with null read, write and hangup handles.
//@ Use `put_read_part` and `put_write_part` to fill in the data transfer directions.
fn null_stream_socket() -> Handle<StreamSocket> {
    Some(StreamSocket {
        read: None,
        write: None,
        close: None,
    })
    .wrap()
}

//@ Create datagram socket with null read, write and hangup handles.
//@ Use `put_source_part` and `put_sink_part` to fill in the data transfer directions.
fn null_datagram_socket() -> Handle<DatagramSocket> {
    Some(DatagramSocket {
        read: None,
        write: None,
        close: None,
    })
    .wrap()
}

//@ Create stream socket with a read handle that emits EOF immediately,
//@ write handle that ignores all incoming data and null hangup handle.
//@
//@ Can also be used a source of dummies for individual directions, with
//@ `take_read_part` and `take_write_part` functions
fn dummy_stream_socket() -> Handle<StreamSocket> {
    Some(StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::empty()),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::empty()),
        }),
        close: None,
    })
    .wrap()
}

struct DummyPkt;

impl PacketRead for DummyPkt {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        Poll::Ready(Ok(PacketReadResult {
            flags: BufferFlag::Eof.into(),
            buffer_subset: 0..0,
        }))
    }
}

impl PacketWrite for DummyPkt {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut [u8],
        _flags: super::types::BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

//@ Create datagram socket with a source handle that continuously emits
//@ EOF-marked empty buffers and a sink  handle that ignores all incoming data
//@ and null hangup handle.
//@
//@ Can also be used a source of dummies for individual directions, with
//@ `take_sink_part` and `take_source_part` functions
fn dummy_datagram_socket() -> Handle<DatagramSocket> {
    Some(DatagramSocket {
        read: Some(DatagramRead {
            src: Box::pin(DummyPkt),
        }),
        write: Some(DatagramWrite {
            snk: Box::pin(DummyPkt),
        }),
        close: None,
    })
    .wrap()
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("read_chunk_limiter", read_chunk_limiter);
    engine.register_fn("null_stream_socket", null_stream_socket);
    engine.register_fn("null_datagram_socket", null_datagram_socket);
    engine.register_fn("dummy_stream_socket", dummy_stream_socket);
    engine.register_fn("dummy_datagram_socket", dummy_datagram_socket);
}
