use std::{ops::Range, task::Poll};

use bytes::BytesMut;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rhai::{Dynamic, Engine, NativeCallContext};
use tinyvec::ArrayVec;
use tokio::io::ReadBuf;
use tracing::{debug, debug_span, trace, warn, Span};
use websocket_sans_io::{
    FrameInfo, Opcode, WebsocketFrameDecoder, WebsocketFrameEncoder, MAX_HEADER_LENGTH,
};

use crate::scenario_executor::utils::ExtractHandleOrFail;

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, PacketRead, PacketReadResult,
        PacketWrite, StreamRead, StreamWrite,
    },
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
    buffer_for_split_control_frames: BytesMut,
}

impl WsEncoder {
    pub fn new(
        span: Span,
        mask_frames: bool,
        flush_after_each_message: bool,
        inner: StreamWrite,
    ) -> WsEncoder {
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
            buffer_for_split_control_frames: BytesMut::new(),
        }
    }
}

#[derive(Debug)]
enum WsEncoderState {
    Idle,
    WritingHeader(ArrayVec<[u8; MAX_HEADER_LENGTH]>),
    WritingData(usize),
    WritingDataFromAltBuffer,
    Flushing,
    Terminating,
    PacketCompleted,
}

impl PacketWrite for WsEncoder {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let _sg = this.span.enter();

        trace!(buflen=buf.len(), "poll_write");
        loop {
            trace!(state=?this.state, "loop");
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
                    if opcode.is_control() && flags.contains(BufferFlag::NonFinalChunk) {
                        this.buffer_for_split_control_frames.extend_from_slice(buf);
                        return Poll::Ready(Ok(()));
                    }
                    if !this.buffer_for_split_control_frames.is_empty() {
                        this.buffer_for_split_control_frames.extend_from_slice(buf);
                    }
                    let fin = !flags.contains(BufferFlag::NonFinalChunk);

                    if opcode.is_data() {
                        this.nonfirst_frame = !fin;
                    }

                    if this.flush_after_each_message && fin {
                        this.flush_pending = true;
                    }
                    if opcode.is_control() && fin {
                        this.flush_pending = true;
                    }
                    let mask = if let Some(ref mut rng) = this.rng_for_mask {
                        Some(rng.gen())
                    } else {
                        None
                    };
                    let mut payload_length = buf.len() as u64;
                    if !this.buffer_for_split_control_frames.is_empty() {
                        payload_length = this.buffer_for_split_control_frames.len() as u64;
                    }
                    let fi = FrameInfo {
                        opcode,
                        payload_length,
                        mask,
                        fin,
                        reserved: 0,
                    };
                    let header = this.fe.start_frame(&fi);
                    this.state = WsEncoderState::WritingHeader(header);
                }
                WsEncoderState::WritingHeader(mut header) => {
                    match tokio::io::AsyncWrite::poll_write(this.inner.writer.as_mut(), cx, &header)
                    {
                        Poll::Ready(Ok(n)) => {
                            let remaining_header = header.split_off(n);
                            if remaining_header.is_empty() {
                                this.fe.transform_frame_payload(buf);
                                if this.buffer_for_split_control_frames.is_empty() {
                                    this.state = WsEncoderState::WritingData(0);
                                } else {
                                    this.state = WsEncoderState::WritingDataFromAltBuffer;
                                }
                            } else {
                                this.state = WsEncoderState::WritingHeader(remaining_header);
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                WsEncoderState::WritingData(offset) => {
                    match tokio::io::AsyncWrite::poll_write(
                        this.inner.writer.as_mut(),
                        cx,
                        &buf[offset..],
                    ) {
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
                WsEncoderState::WritingDataFromAltBuffer => {
                    match tokio::io::AsyncWrite::poll_write(
                        this.inner.writer.as_mut(),
                        cx,
                        &this.buffer_for_split_control_frames,
                    ) {
                        Poll::Ready(Ok(n)) => {
                            let _ = this.buffer_for_split_control_frames.split_to(n);
                            if this.buffer_for_split_control_frames.is_empty() {
                                this.state = WsEncoderState::Flushing;
                            } else {
                                this.state = WsEncoderState::WritingDataFromAltBuffer;
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
                    return Poll::Ready(Ok(()));
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

    let x = WsEncoder::new(
        span.clone(),
        opts.masked,
        !opts.no_flush_after_each_message,
        inner,
    );
    let x = DatagramWrite { snk: Box::pin(x) };
    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

struct WsDecoder {
    span: Span,
    inner: StreamRead,
    require_masked: bool,
    require_unmasked: bool,
    wd: WebsocketFrameDecoder,
    /// Bytes that were read in the buffer, but not returned as a part of packet and should be
    /// reused by next `poll_read` invocation instead of reading from `inner` again.
    unprocessed_bytes: usize,
    offset: usize,
}

impl PacketRead for WsDecoder {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();
        let _sg = this.span.enter();

        macro_rules! invdata {
            () => {
                return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()))
            };
        }

        let mut outflags: BufferFlags = BufferFlag::NonFinalChunk.into();
        let mut outrange: Option<Range<usize>> = None;

        macro_rules! fill_in_flags_based_on_opcode {
            ($original_opcode:expr) => {
                match $original_opcode {
                    Opcode::Continuation => (),
                    Opcode::Text => outflags |= BufferFlag::Text,
                    Opcode::Binary => (),
                    Opcode::ConnectionClose => outflags |= BufferFlag::Eof,
                    Opcode::Ping => outflags |= BufferFlag::Ping,
                    Opcode::Pong => outflags |= BufferFlag::Pong,
                    _ => (),
                }
            };
        }

        #[derive(Debug)]
        enum DataSource {
            FromPrefix,
            FromBuf,
        }

        trace!("poll_read");
        let mut need_to_issue_inner_read = false;
        loop {
            let mut pending_exit_from_loop = false;
            let (ds, unprocessed_data_range) = if !this.inner.prefix.is_empty() {
                (
                    DataSource::FromPrefix,
                    0..buf.len().min(this.inner.prefix.len()),
                )
            } else {
                if this.unprocessed_bytes > 0 || !need_to_issue_inner_read {
                    assert!(this.unprocessed_bytes <= buf.len());
                    (
                        DataSource::FromBuf,
                        this.offset..(this.offset + this.unprocessed_bytes),
                    )
                } else {
                    trace!("inner read");
                    let mut rb = ReadBuf::new(&mut buf[this.offset..]);
                    match tokio::io::AsyncRead::poll_read(this.inner.reader.as_mut(), cx, &mut rb) {
                        Poll::Ready(Ok(())) => {
                            this.unprocessed_bytes = rb.filled().len();
                            if this.unprocessed_bytes == 0 {
                                outflags |= BufferFlag::Eof;
                                outflags &= !BufferFlag::NonFinalChunk;
                                pending_exit_from_loop = true;
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    };
                    need_to_issue_inner_read = false;
                    (
                        DataSource::FromBuf,
                        this.offset..(this.offset + this.unprocessed_bytes),
                    )
                }
            };
            trace!(?ds, range=?unprocessed_data_range.clone(), "data ready");

            let unprocessed_data: &mut [u8] = match ds {
                DataSource::FromPrefix => &mut this.inner.prefix[unprocessed_data_range.clone()],
                DataSource::FromBuf => &mut buf[unprocessed_data_range.clone()],
            };

            let ret = this.wd.add_data(unprocessed_data);

            trace!(?ret, "decoded");
            let Ok(ret) = ret else { invdata!() };

            match ret.event {
                None => {
                    if ret.consumed_bytes == 0 {
                        if outrange.is_some() {
                            pending_exit_from_loop = true;
                        } else {
                            need_to_issue_inner_read = true;
                        }
                    }
                }
                Some(websocket_sans_io::WebsocketFrameEvent::Start {
                    frame_info,
                    original_opcode,
                }) => {
                    if !frame_info.is_reasonable() {
                        warn!("Invalid WebSocket frame header: {frame_info:?}");
                        invdata!();
                    }
                    if this.require_masked && frame_info.mask.is_none() {
                        warn!("Unmasked frame where masked expected");
                        invdata!();
                    }
                    if this.require_unmasked && frame_info.mask.is_some() {
                        warn!("Masked frame where unmasked is expected");
                        invdata!();
                    }
                    fill_in_flags_based_on_opcode!(original_opcode);
                }
                Some(websocket_sans_io::WebsocketFrameEvent::PayloadChunk { original_opcode }) => {
                    fill_in_flags_based_on_opcode!(original_opcode);
                    assert!(outrange.is_none());
                    match ds {
                        DataSource::FromPrefix => {
                            outrange = Some(0..ret.consumed_bytes);
                            buf[0..ret.consumed_bytes]
                                .copy_from_slice(&this.inner.prefix[0..ret.consumed_bytes]);
                        }
                        DataSource::FromBuf => {
                            outrange = Some(this.offset..(this.offset + ret.consumed_bytes));
                        }
                    }
                }
                Some(websocket_sans_io::WebsocketFrameEvent::End {
                    frame_info,
                    original_opcode,
                }) => {
                    fill_in_flags_based_on_opcode!(original_opcode);
                    if frame_info.fin {
                        outflags &= !BufferFlag::NonFinalChunk;
                    }
                    pending_exit_from_loop = true;
                }
            }
            match ds {
                DataSource::FromPrefix => {
                    let _ = this.inner.prefix.split_to(ret.consumed_bytes);
                    if this.inner.prefix.is_empty() {
                        trace!("fully processed read prefix")
                    }
                }
                DataSource::FromBuf => {
                    this.offset += ret.consumed_bytes;
                    this.unprocessed_bytes -= ret.consumed_bytes;
                    if this.unprocessed_bytes == 0 {
                        trace!("fully processed this read chunk")
                    }
                }
            }
            if pending_exit_from_loop {
                break;
            }
        }

        let r = PacketReadResult {
            flags: outflags,
            buffer_subset: outrange.unwrap_or(0..0),
        };
        debug!(?r, "frame ready");
        Poll::Ready(Ok(r))
    }
}

impl WsDecoder {
    pub fn new(
        span: Span,
        inner: StreamRead,
        require_masked: bool,
        require_unmasked: bool,
    ) -> WsDecoder {
        WsDecoder {
            span,
            inner,
            require_masked,
            require_unmasked,
            wd: WebsocketFrameDecoder::new(),
            unprocessed_bytes: 0,
            offset: 0,
        }
    }
}

fn ws_decoder(
    _ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamRead>,
) -> RhResult<Handle<DatagramRead>> {
    let span = debug_span!("ws_decoder");
    #[derive(serde::Deserialize)]
    struct WsDecoderOpts {
        #[serde(default)]
        require_masked: bool,
        #[serde(default)]
        require_unmasked: bool,
    }
    let opts: WsDecoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = inner.lutbar()?;
    debug!(parent: &span, inner=?inner, "options parsed");

    let x = WsDecoder::new(
        span.clone(),
        inner,
        opts.require_masked,
        opts.require_unmasked,
    );
    let x = DatagramRead { src: Box::pin(x) };
    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_encoder", ws_encoder);
    engine.register_fn("ws_decoder", ws_decoder);
}
