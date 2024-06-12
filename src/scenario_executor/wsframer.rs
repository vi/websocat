use std::task::Poll;

use rand::{rngs::StdRng, Rng, SeedableRng};
use rhai::{Dynamic, Engine, NativeCallContext};
use tinyvec::ArrayVec;
use tracing::{debug, debug_span, Span};
use websocket_sans_io::{FrameInfo, Opcode, WebsocketFrameEncoder, MAX_HEADER_LENGTH};

use crate::scenario_executor::utils::ExtractHandleOrFail;

use super::{
    types::{BufferFlag, DatagramWrite, Handle, PacketWrite, StreamWrite},
    utils::RhResult,
};

pub struct WsEncoder {
    inner: StreamWrite,
    span: Span,
    rng_for_mask: Option<StdRng>,
    fe: WebsocketFrameEncoder,
    state: WsEncoderState,
    flush_after_each_message: bool,
    flush_pending: bool,
    terminate_pending: bool,
    nonfirst_frame: bool,
}

impl WsEncoder {
    pub fn new(span: Span, mask_frames: bool, flush_after_each_message: bool, inner: StreamWrite) -> WsEncoder {

        let rng_for_mask = if mask_frames {
            Some(StdRng::from_rng(rand::thread_rng()).unwrap())
        } else {
            None
        };

        WsEncoder {
            span,
            inner,
            rng_for_mask,
            fe: WebsocketFrameEncoder::new(),
            state: WsEncoderState::Idle,
            flush_after_each_message,
            flush_pending: false,
            terminate_pending: false,
            nonfirst_frame: false,
        }
    }
}

enum WsEncoderState {
    Idle,
    WritingHeader(ArrayVec<[u8; MAX_HEADER_LENGTH]>),
    WritingData(usize),
    Flushing,
    Terminating,
    PacketCompleted,
}

impl PacketWrite for WsEncoder {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
        flags: super::types::BufferFlags,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let _sg = this.span.enter();

        let buf = buf.filled_mut();

        loop {
            match this.state {
                WsEncoderState::Idle => {
                    let mut opcode = Opcode::Binary;
                    if flags.contains(BufferFlag::Text) {
                        opcode = Opcode::Text;
                    }
                    if this.nonfirst_frame {
                        opcode = Opcode::Continuation;
                    }
                    if flags.contains(BufferFlag::Ping) {
                        opcode = Opcode::Ping;
                    }
                    if flags.contains(BufferFlag::Pong) {
                        opcode = Opcode::Pong;
                    }
                    if flags.contains(BufferFlag::Eof) {
                        debug!("EOF encountered");
                        opcode = Opcode::ConnectionClose;
                        this.terminate_pending = true;
                    }
                    let fin = !flags.contains(BufferFlag::NonFinalChunk);

                    if opcode.is_data() {
                        this.nonfirst_frame = !fin;
                    }

                    if this.flush_after_each_message && fin {
                        this.flush_pending = true;
                    }
                    let mask = if let Some(ref mut rng) = this.rng_for_mask {
                        Some(rng.gen())
                    } else {
                        None
                    };
                    let fi = FrameInfo {
                        opcode,
                        payload_length: buf.len() as u64,
                        mask,
                        fin,
                        reserved: 0,
                    };
                    let header = this.fe.start_frame(&fi);
                    this.state = WsEncoderState::WritingHeader(header);
                }
                WsEncoderState::WritingHeader(mut header) => {
                    match tokio::io::AsyncWrite::poll_write(this.inner.writer.as_mut(), cx, &header) {
                        Poll::Ready(Ok(n)) => {
                            let remaining_header = header.split_off(n);
                            if remaining_header.is_empty() {
                                this.fe.transform_frame_payload(buf);
                                this.state = WsEncoderState::WritingData(0);
                            } else {
                                this.state = WsEncoderState::WritingHeader(remaining_header);
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                WsEncoderState::WritingData(offset) => {
                    match tokio::io::AsyncWrite::poll_write(this.inner.writer.as_mut(), cx, &buf[offset..]) {
                        Poll::Ready(Ok(n)) => {
                            let new_offset = offset + n;
                            if new_offset == buf.len() {
                                if this.terminate_pending {
                                    this.state = WsEncoderState::Terminating;
                                } else if this.flush_pending {
                                    this.state = WsEncoderState::Flushing;
                                } else {
                                    this.state = WsEncoderState::PacketCompleted;
                                }
                            } else {
                                this.state = WsEncoderState::WritingData(new_offset);
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                WsEncoderState::Flushing => {
                    match tokio::io::AsyncWrite::poll_flush(this.inner.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => {
                            this.state = WsEncoderState::PacketCompleted;
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                WsEncoderState::Terminating => {
                    match tokio::io::AsyncWrite::poll_shutdown(this.inner.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => {
                            debug!("shutdown completed");
                            this.state = WsEncoderState::PacketCompleted;
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                WsEncoderState::PacketCompleted => {
                    this.flush_pending = false;
                    this.terminate_pending = false;
                    this.state = WsEncoderState::Idle;
                    return Poll::Ready(Ok(()))
                }
            }
        }
    }
}

fn ws_encoder(
    _ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamWrite>,
) -> RhResult<Handle<DatagramWrite>> {
    let span = debug_span!("ws_encoder");
    #[derive(serde::Deserialize)]
    struct WsEncoderOpts {
        masked: bool,
        #[serde(default)]
        no_flush_after_each_message: bool,
    }
    let opts: WsEncoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = inner.lutbar()?;
    debug!(parent: &span, inner=?inner, "options parsed");

    let x = WsEncoder::new(span.clone(),  opts.masked, ! opts.no_flush_after_each_message, inner);
    let x = DatagramWrite { snk: Box::pin(x) };
    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_encoder", ws_encoder);
}
