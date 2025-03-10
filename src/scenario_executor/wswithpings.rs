use std::{
    io::ErrorKind,
    pin::Pin,
    sync::Mutex,
    task::{ready, Poll},
};

use rand::SeedableRng;
use rhai::{Dynamic, Engine, NativeCallContext};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::PollSemaphore;
use tracing::{debug, debug_span, trace};

use crate::scenario_executor::{
    scenario::ScenarioAccess,
    types::StreamWrite,
    utils1::{ExtractHandleOrFail, SimpleErr},
    utils2::PollSemaphoreNew2,
    wsframer::{WsDecoder, WsEncoder},
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite, StreamSocket,
    },
    utils1::RhResult,
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
            match ready!(inner.sem.poll_acquire(cx)) {
                None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Some(p) => this.sem_permit = Some(p),
            }
        }

        let ret = ready!(PacketWrite::poll_write(
            Pin::new(&mut inner.inner),
            cx,
            buf,
            flags
        ));
        this.sem_permit = None;
        Poll::Ready(ret)
    }
}

struct WsDecoderThatCoexistsWithPingReplies {
    inner: WsDecoder,
    writer: Arc<Mutex<WsEncoderThatCoexistsWithPongs>>,
    /// Permit to finish writing series of frames that will be assembled as full Pong frame
    sem_permit: Option<OwnedSemaphorePermit>,
    ping_reply_in_progress: Option<PacketReadResult>,
    pong_replies_limit: Option<usize>,
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
                    match ready!(writer.sem.poll_acquire(cx)) {
                        None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                        Some(p) => this.sem_permit = Some(p),
                    }
                }

                let rb = &mut buf[prip.buffer_subset.clone()];

                let flags = prip.flags & !BufferFlag::Ping | BufferFlag::Pong;
                ready!(PacketWrite::poll_write(
                    Pin::new(&mut writer.inner),
                    cx,
                    rb,
                    flags
                ))?;
                this.ping_reply_in_progress = None;
                if prip.flags.contains(BufferFlag::NonFinalChunk) {
                    trace!(
                        "ping reply split in middle - not unlocking normal traffic to WebSocket"
                    );
                } else {
                    debug!("Replies to a ping");
                    this.sem_permit = None;
                }
            }

            let ret = ready!(PacketRead::poll_read(Pin::new(&mut this.inner), cx, buf))?;
            if ret.flags.contains(BufferFlag::Ping) {
                let reply_to_this_ping = if let Some(ref mut pl) = this.pong_replies_limit {
                    if *pl == 0 {
                        debug!("Inhibiting this WebSocket ping reply due to --inhibit-pongs limit");
                        false
                    } else {
                        *pl -= 1;
                        true
                    }
                } else {
                    true
                };

                if reply_to_this_ping {
                    trace!("ping detected, replying instead of passing upstream");
                    this.ping_reply_in_progress = Some(ret);
                    continue;
                }
            }

            return Poll::Ready(Ok(ret));
        }
    }
}

//@ Like ws_encoder + ws_decoder, but also set up automatic replier to WebSocket pings.
fn ws_wrap(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("ws_wrap");
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Mask outgoing frames and require unmasked incoming frames
        client: bool,

        //@ Accept masked (unmasked) frames in client (server) mode.
        #[serde(default)]
        ignore_masks: bool,

        //@ Inhibit flushing of underlying stream writer after each compelte message
        #[serde(default)]
        no_flush_after_each_message: bool,

        //@ Do not emit ConnectionClose frame when writing part is getting shut down
        #[serde(default)]
        no_close_frame: bool,

        //@ Propagate upstream writer shutdown to downstream
        #[serde(default)]
        shutdown_socket_on_eof: bool,

        //@ Do not automatically wrap WebSocket frames writer
        //@ in a write_buffer: overlay when it detects missing
        //@ vectored writes support
        #[serde(default)]
        no_auto_buffer_wrap: bool,

        //@ Stop replying to WebSocket pings after sending this number of Pong frames.
        max_ping_replies: Option<usize>,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let StreamSocket {
        read,
        write,
        close,
        fd,
    } = inner;

    let (Some(inner_read), Some(inner_write)) = (read, write) else {
        return Err(ctx.err("Incomplete stream socket"));
    };

    let (require_masked, require_unmasked) = if opts.ignore_masks {
        (false, false)
    } else if opts.client {
        (false, true)
    } else {
        (true, false)
    };

    let mut maybe_buffered_write = inner_write;

    if !opts.no_auto_buffer_wrap && !maybe_buffered_write.writer.is_write_vectored() {
        maybe_buffered_write = StreamWrite {
            writer: Box::pin(tokio::io::BufWriter::new(maybe_buffered_write.writer)),
        }
    }

    let rng = if opts.client {
        let the_scenario = ctx.get_scenario()?;
        let prng = rand_pcg::Pcg64::from_rng(&mut *the_scenario.prng.lock().unwrap());
        Some(prng)
    } else {
        None
    };

    let usual_encoder = WsEncoder::new(
        span.clone(),
        rng,
        !opts.no_flush_after_each_message,
        maybe_buffered_write,
        !opts.no_close_frame,
        opts.shutdown_socket_on_eof,
    );
    let usuad_decoder = WsDecoder::new(span.clone(), inner_read, require_masked, require_unmasked);

    let x = if opts.max_ping_replies == Some(0) {
        DatagramSocket {
            read: Some(DatagramRead {
                src: Box::pin(usuad_decoder),
            }),
            write: Some(DatagramWrite {
                snk: Box::pin(usual_encoder),
            }),
            close,
            fd,
        }
    } else {
        let shared_encoder = WsEncoderThatCoexistsWithPongs {
            inner: usual_encoder,
            sem: PollSemaphore::new2(1),
        };
        let shared_encoder = Arc::new(Mutex::new(shared_encoder));

        let shared_decoder = WsDecoderThatCoexistsWithPingReplies {
            inner: usuad_decoder,
            writer: shared_encoder.clone(),
            ping_reply_in_progress: None,
            sem_permit: None,
            pong_replies_limit: opts.max_ping_replies,
        };
        let dr = DatagramRead {
            src: Box::pin(shared_decoder),
        };

        let e = WsEncoderThatCoexistsWithPongsHandle {
            inner: shared_encoder,
            sem_permit: None,
        };
        let dw = DatagramWrite { snk: Box::pin(e) };

        DatagramSocket {
            read: Some(dr),
            write: Some(dw),
            close,
            fd,
        }
    };

    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_wrap", ws_wrap);
}
