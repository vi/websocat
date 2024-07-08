use std::{pin::Pin, task::Poll};

use pin_project::pin_project;
use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::io::ReadBuf;
use tracing::{debug, debug_span, error, field, Instrument};

use crate::scenario_executor::utils::{ExtractHandleOrFail, SimpleErr};

use super::{types::{BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead, PacketReadResult, PacketWrite, StreamRead, StreamSocket, StreamWrite}, utils::RhResult};


#[pin_project]
struct ReadStreamChunks(#[pin] StreamRead);

impl PacketRead for ReadStreamChunks {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let sr: Pin<&mut StreamRead> = self.project().0;

        let mut rb = ReadBuf::new(buf);

        match tokio::io::AsyncRead::poll_read(sr, cx, &mut rb) {
            Poll::Ready(Ok(())) => {
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
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn read_line_chunks(
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
struct WriteStreamChunks {
    w: StreamWrite,
    debt: usize,
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
                    match tokio::io::AsyncWrite::poll_flush(sw.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                if flags.contains(BufferFlag::Eof) {
                    match tokio::io::AsyncWrite::poll_shutdown(sw.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                *p.debt = 0;
                break;
            }
            match tokio::io::AsyncWrite::poll_write(sw.writer.as_mut(), cx, buf_chunk) {
                Poll::Ready(Ok(n)) => {
                    *p.debt += n;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        return Poll::Ready(Ok(()));
    }
}

fn write_line_chunks(
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

fn line_chunks(
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
    engine.register_fn("write_line_chunks", read_line_chunks);
    engine.register_fn("read_line_chunks", write_line_chunks);
    engine.register_fn("line_chunks", line_chunks);
}
