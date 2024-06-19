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

pub fn register(engine: &mut Engine) {
    engine.register_fn("read_chunk_limiter", read_chunk_limiter);
}
