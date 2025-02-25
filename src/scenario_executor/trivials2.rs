use std::{
    pin::Pin,
    task::{ready, Poll},
};

use base64::Engine as _;
use bytes::BytesMut;
use pin_project::pin_project;
use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::debug;

use crate::scenario_executor::{
    logoverlay::render_content,
    types::{Handle, StreamRead},
    utils1::{ExtractHandleOrFail, RhResult},
};

use super::{
    scenario::ScenarioAccess,
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, PacketRead,
        PacketReadResult, PacketWrite, StreamSocket, StreamWrite,
    },
    utils1::{HandleExt, SimpleErr},
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

        buf.initialize_unfilled();
        let b = buf.initialized_mut();
        let limit = b.len().min(*this.limit);
        let b = &mut b[0..limit];
        let mut rb = ReadBuf::new(b);

        ready!(tokio::io::AsyncRead::poll_read(this.inner, cx, &mut rb))?;
        let read_len = rb.filled().len();
        buf.advance(read_len);
        Poll::Ready(Ok(()))
    }
}

//@ Transform stream source so that reads become short reads if there is enough data. For development and testing.
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

struct WriteChunkLimiter {
    inner: StreamWrite,
    limit: usize,
}

impl AsyncWrite for WriteChunkLimiter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        if buf.len() > this.limit {
            buf = &buf[..this.limit];
        }
        AsyncWrite::poll_write(Pin::new(&mut this.inner.writer), cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        AsyncWrite::poll_flush(Pin::new(&mut this.inner.writer), cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        AsyncWrite::poll_shutdown(Pin::new(&mut this.inner.writer), cx)
    }
}

//@ Transform stream sink so that writes become short writes if the buffer is too large. For development and testing.
fn write_chunk_limiter(
    ctx: NativeCallContext,
    x: Handle<StreamWrite>,
    limit: i64,
) -> RhResult<Handle<StreamWrite>> {
    let x = ctx.lutbar(x)?;
    debug!(inner=?x, "write_chunk_limiter");
    let x = StreamWrite {
        writer: Box::pin(WriteChunkLimiter {
            inner: x,
            limit: limit as usize,
        }),
    };
    debug!(wrapped=?x, "write_chunk_limiter");
    Ok(x.wrap())
}

#[allow(unused)] // TODO: expose this
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

        ready!(tokio::io::AsyncRead::poll_read(
            sr.reader.as_mut(),
            cx,
            &mut rb
        ))?;
        let read_len = rb.filled().len();
        buf.advance(read_len);
        Poll::Ready(Ok(()))
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

//@ Wrap stream writer in a buffering overlay that may accumulate data,
//@ e.g. to write in one go on flush
fn write_buffer(
    ctx: NativeCallContext,
    inner: Handle<StreamWrite>,
    capacity: i64,
) -> RhResult<Handle<StreamWrite>> {
    Ok(Some(StreamWrite {
        writer: Box::pin(tokio::io::BufWriter::with_capacity(
            capacity as usize,
            ctx.lutbar(inner)?.writer,
        )),
    })
    .wrap())
}

//@ Decode base64 string to another string
fn b64str(ctx: NativeCallContext, x: &str) -> RhResult<String> {
    let Ok(buf) = base64::prelude::BASE64_STANDARD.decode(x) else {
        return Err(ctx.err("Failed to base64-decode the argument"));
    };
    let Ok(s) = String::from_utf8(buf) else {
        return Err(ctx.err("Base64-encoded content is not a valid UTF-8"));
    };
    Ok(s)
}

//@ Debug print something to stderr
fn debug_print(ctx: NativeCallContext, x: Dynamic) -> RhResult<()> {
    let the_scenario = ctx.get_scenario()?;
    let mut diago = the_scenario.diagnostic_output.lock().unwrap();
    if x.is_blob() {
        let b = x.into_blob().unwrap();
        let _ = writeln!(diago, "b{}", render_content(&b, false));
    } else {
        let _ = writeln!(diago, "{:?}", x);
    }
    Ok(())
}


//@ Print a string to stdout (synchronously)
fn print_stdout(x: &str) {
    print!("{x}");
}

//@ Create a stream socket with a read handle emits specified data, then EOF; and
//@ write handle that ignores all incoming data and null hangup handle.
fn literal_socket(data: String) -> Handle<StreamSocket> {
    Some(StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::empty()),
            prefix: BytesMut::from(data.as_bytes()),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::empty()),
        }),
        close: None,
    })
    .wrap()
}

//@ Create a stream socket with a read handle emits specified data, then EOF; and
//@ write handle that ignores all incoming data and null hangup handle.
fn literal_socket_base64(ctx: NativeCallContext, data: String) -> RhResult<Handle<StreamSocket>> {
    let Ok(d) = base64::prelude::BASE64_STANDARD.decode(data) else {
        return Err(ctx.err("Invalid base64 data"));
    };
    Ok(Some(StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::empty()),
            prefix: BytesMut::from(&d[..]),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::empty()),
        }),
        close: None,
    })
    .wrap())
}

#[pin_project]
pub struct ReadStreamChunks(#[pin] pub StreamRead);

impl PacketRead for ReadStreamChunks {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let sr: Pin<&mut StreamRead> = self.project().0;

        let mut rb = ReadBuf::new(buf);

        ready!(tokio::io::AsyncRead::poll_read(sr, cx, &mut rb))?;
        let new_len = rb.filled().len();
        let flags = if new_len > 0 {
            BufferFlags::default()
        } else {
            BufferFlag::Eof.into()
        };
        Poll::Ready(Ok(PacketReadResult {
            flags,
            buffer_subset: 0..new_len,
        }))
    }
}

//@ Convert a stream source to a datagram source
fn read_stream_chunks(
    ctx: NativeCallContext,
    x: Handle<StreamRead>,
) -> RhResult<Handle<DatagramRead>> {
    let x = ctx.lutbar(x)?;
    debug!(inner=?x, "read_stream_chunks");
    let x = DatagramRead {
        src: Box::pin(ReadStreamChunks(x)),
    };
    debug!(wrapped=?x, "read_stream_chunks");
    Ok(x.wrap())
}

#[pin_project]
pub struct WriteStreamChunks {
    pub w: StreamWrite,
    pub debt: usize,
}

impl PacketWrite for WriteStreamChunks {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let p = self.project();
        let sw: &mut StreamWrite = p.w;

        loop {
            assert!(buf.len() >= *p.debt);
            let buf_chunk = &buf[*p.debt..];
            if buf_chunk.is_empty() {
                if !flags.contains(BufferFlag::NonFinalChunk) {
                    ready!(tokio::io::AsyncWrite::poll_flush(sw.writer.as_mut(), cx))?;
                }
                if flags.contains(BufferFlag::Eof) {
                    ready!(tokio::io::AsyncWrite::poll_shutdown(sw.writer.as_mut(), cx))?;
                }
                *p.debt = 0;
                break;
            }
            let n = ready!(tokio::io::AsyncWrite::poll_write(
                sw.writer.as_mut(),
                cx,
                buf_chunk
            ))?;
            *p.debt += n;
        }
        return Poll::Ready(Ok(()));
    }
}

//@ Convert a stream sink to a datagram sink
fn write_stream_chunks(
    ctx: NativeCallContext,
    x: Handle<StreamWrite>,
) -> RhResult<Handle<DatagramWrite>> {
    let x = ctx.lutbar(x)?;
    debug!(inner=?x, "write_stream_chunks");
    let x = DatagramWrite {
        snk: Box::pin(WriteStreamChunks { w: x, debt: 0 }),
    };
    debug!(wrapped=?x, "write_stream_chunks");
    Ok(x.wrap())
}

//@ Convert a stream socket to a datagram socket. Like write_stream_chunks + read_stream_chunks while also preserving the hangup signal.
fn stream_chunks(
    ctx: NativeCallContext,
    x: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let x = ctx.lutbar(x)?;
    debug!(inner=?x, "stream_chunks");

    if let StreamSocket {
        read: Some(r),
        write: Some(w),
        close,
    } = x
    {
        let write = DatagramWrite {
            snk: Box::pin(WriteStreamChunks { w: w, debt: 0 }),
        };
        let read = DatagramRead {
            src: Box::pin(ReadStreamChunks(r)),
        };
        let x = DatagramSocket {
            read: Some(read),
            write: Some(write),
            close,
        };
        debug!(wrapped=?x, "stream_chunks");
        Ok(x.wrap())
    } else {
        Err(ctx.err(""))
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("read_chunk_limiter", read_chunk_limiter);
    engine.register_fn("write_chunk_limiter", write_chunk_limiter);
    engine.register_fn("null_stream_socket", null_stream_socket);
    engine.register_fn("null_datagram_socket", null_datagram_socket);
    engine.register_fn("dummy_stream_socket", dummy_stream_socket);
    engine.register_fn("dummy_datagram_socket", dummy_datagram_socket);
    engine.register_fn("write_buffer", write_buffer);
    engine.register_fn("b64str", b64str);
    engine.register_fn("dbg", debug_print);
    engine.register_fn("print_stdout", print_stdout);
    engine.register_fn("literal_socket", literal_socket);
    engine.register_fn("literal_socket_base64", literal_socket_base64);
    engine.register_fn("read_stream_chunks", read_stream_chunks);
    engine.register_fn("write_stream_chunks", write_stream_chunks);
    engine.register_fn("stream_chunks", stream_chunks);
}
