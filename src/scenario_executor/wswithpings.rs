use std::{io::ErrorKind, pin::Pin, sync::Mutex, task::Poll};

use rhai::{Dynamic, Engine, NativeCallContext};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::PollSemaphore;
use tracing::{debug, debug_span, trace};

use crate::scenario_executor::{
    types::StreamWrite,
    utils::{ExtractHandleOrFail, SimpleErr},
    wsframer::{WsDecoder, WsEncoder},
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite, StreamSocket,
    },
    utils::RhResult,
};

struct WsEncoderThatCoexistsWithPongs {
    inner: WsEncoder,
    sem: PollSemaphore,
}

struct WsEncoderThatCoexistsWithPongsHandle {
    inner: Arc<Mutex<WsEncoderThatCoexistsWithPongs>>,
    /// Permit to finish up writing one (probably non-Pong) frame to WebSocket.
    sem_permit: Option<OwnedSemaphorePermit>,
}

impl PacketWrite for WsEncoderThatCoexistsWithPongsHandle {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        let mut inner = this.inner.lock().unwrap();
        if this.sem_permit.is_none() {
            match inner.sem.poll_acquire(cx) {
                Poll::Ready(None) => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Poll::Ready(Some(p)) => this.sem_permit = Some(p),
                Poll::Pending => return Poll::Pending,
            }
        }

        match PacketWrite::poll_write(Pin::new(&mut inner.inner), cx, buf, flags) {
            Poll::Ready(ret) => {
                this.sem_permit = None;
                Poll::Ready(ret)
            }
            Poll::Pending => return Poll::Pending,
        }
    }
}

struct WsDecoderThatCoexistsWithPingReplies {
    inner: WsDecoder,
    writer: Arc<Mutex<WsEncoderThatCoexistsWithPongs>>,
    /// Permit to finish writing series of frames that will be assembled as full Pong frame
    sem_permit: Option<OwnedSemaphorePermit>,
    ping_reply_in_progress: Option<PacketReadResult>,
}

impl PacketRead for WsDecoderThatCoexistsWithPingReplies {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();

        loop {
            if let Some(prip) = this.ping_reply_in_progress.clone() {
                let mut writer = this.writer.lock().unwrap();
                if this.sem_permit.is_none() {
                    match writer.sem.poll_acquire(cx) {
                        Poll::Ready(None) => {
                            return Poll::Ready(Err(ErrorKind::ConnectionReset.into()))
                        }
                        Poll::Ready(Some(p)) => this.sem_permit = Some(p),
                        Poll::Pending => return Poll::Pending,
                    }
                }

                let rb = &mut buf[prip.buffer_subset.clone()];

                let flags = prip.flags & !BufferFlag::Ping | BufferFlag::Pong;
                match PacketWrite::poll_write(Pin::new(&mut writer.inner), cx, rb, flags) {
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Ready(Ok(())) => {
                        this.ping_reply_in_progress = None;
                        if prip.flags.contains(BufferFlag::NonFinalChunk) {
                            trace!("ping reply split in middle - not unlocking normal traffic to WebSocket");
                        } else {
                            debug!("Replies to a ping");
                            this.sem_permit = None;
                        }
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            return match PacketRead::poll_read(Pin::new(&mut this.inner), cx, buf) {
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(ret)) => {
                    if ret.flags.contains(BufferFlag::Ping) {
                        trace!("ping detected, replying instead of passing upstream");
                        this.ping_reply_in_progress = Some(ret);
                        continue;
                    }

                    Poll::Ready(Ok(ret))
                }
                Poll::Pending => Poll::Pending,
            };
        }
    }
}

fn ws_wrap(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("ws_wrap");
    #[derive(serde::Deserialize)]
    struct WsDecoderOpts {
        client: bool,
        #[serde(default)]
        ignore_masks: bool,
        #[serde(default)]
        no_flush_after_each_message: bool,

        #[serde(default)]
        no_close_frame: bool,

        #[serde(default)]
        shutdown_socket_on_eof: bool,

        //@ Do not automatically wrap WebSocket frames writer
        //@ in a write_buffer: overlay when it detects missing
        //@ vectored writes support
        #[serde(default)]
        no_auto_buffer_wrap: bool,
    }
    let opts: WsDecoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let StreamSocket { read, write, close } = inner;

    let (Some(inner_read), Some(inner_write)) = (read, write) else {
        return Err(ctx.err("Incomplete stream socket"));
    };

    let (require_masked, require_unmasked) = if opts.ignore_masks {
        (false, false)
    } else {
        if opts.client {
            (false, true)
        } else {
            (true, false)
        }
    };

    let mut maybe_buffered_write = inner_write;

    if !opts.no_auto_buffer_wrap && !maybe_buffered_write.writer.is_write_vectored() {
        maybe_buffered_write = StreamWrite {
            writer: Box::pin(tokio::io::BufWriter::new(maybe_buffered_write.writer)),
        }
    }

    let usual_encoder = WsEncoder::new(
        span.clone(),
        opts.client,
        !opts.no_flush_after_each_message,
        maybe_buffered_write,
        !opts.no_close_frame,
        opts.shutdown_socket_on_eof,
    );

    let shared_encoder = WsEncoderThatCoexistsWithPongs {
        inner: usual_encoder,
        sem: PollSemaphore::new(Arc::new(tokio::sync::Semaphore::new(1))),
    };
    let shared_encoder = Arc::new(Mutex::new(shared_encoder));

    let d = WsDecoder::new(span.clone(), inner_read, require_masked, require_unmasked);
    let dd = WsDecoderThatCoexistsWithPingReplies {
        inner: d,
        writer: shared_encoder.clone(),
        ping_reply_in_progress: None,
        sem_permit: None,
    };
    let dr = DatagramRead { src: Box::pin(dd) };

    let e = WsEncoderThatCoexistsWithPongsHandle {
        inner: shared_encoder,
        sem_permit: None,
    };
    let dw = DatagramWrite { snk: Box::pin(e) };

    let x = DatagramSocket {
        read: Some(dr),
        write: Some(dw),
        close,
    };

    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_wrap", ws_wrap);
}
