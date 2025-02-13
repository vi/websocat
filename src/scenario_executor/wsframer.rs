use std::{io::IoSlice, ops::Range, task::Poll};

use bytes::BytesMut;
use pin_project::pin_project;
use rand::{Rng, SeedableRng};
use rhai::{Dynamic, Engine, NativeCallContext};
use tinyvec::ArrayVec;
use tokio::io::ReadBuf;
use tracing::{debug, debug_span, trace, warn, Span};
use websocket_sans_io::{
    FrameInfo, Opcode, WebsocketFrameDecoder, WebsocketFrameEncoder, MAX_HEADER_LENGTH,
};

use crate::scenario_executor::{scenario::ScenarioAccess, utils1::ExtractHandleOrFail};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, PacketRead, PacketReadResult,
        PacketWrite, StreamRead, StreamWrite,
    },
    utils1::RhResult,
};

pub struct WsEncoder {
    inner: StreamWrite,
    span: Span,
    rng_for_mask: Option<rand_pcg::Pcg64>,
    fe: WebsocketFrameEncoder,
    state: WsEncoderState,
    flush_after_each_message: bool,
    flush_pending: bool,
    terminate_pending: bool,
    nonfirst_frame: bool,
    buffer_for_split_control_frames: BytesMut,
    send_close_frame_on_eof: bool,
    shutdown_socket_on_eof: bool,
}

impl WsEncoder {
    pub fn new(
        span: Span,
        mask_frames: Option<rand_pcg::Pcg64>,
        flush_after_each_message: bool,
        inner: StreamWrite,
        send_close_frame_on_eof: bool,
        shutdown_socket_on_eof: bool,
    ) -> WsEncoder {
        WsEncoder {
            span,
            inner,
            rng_for_mask: mask_frames,
            fe: WebsocketFrameEncoder::new(),
            state: WsEncoderState::Idle,
            flush_after_each_message,
            flush_pending: false,
            terminate_pending: false,
            nonfirst_frame: false,
            buffer_for_split_control_frames: BytesMut::new(),
            send_close_frame_on_eof,
            shutdown_socket_on_eof,
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

        trace!(buflen = buf.len(), "poll_write");
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
                        if this.send_close_frame_on_eof {
                            opcode = Opcode::ConnectionClose;
                            this.terminate_pending = true;
                        } else {
                            debug!("Not sending the close frame");
                            this.state = WsEncoderState::Terminating;
                            continue;
                        }
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
                        Some(rng.random())
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
                    this.fe.transform_frame_payload(buf);
                    this.state = WsEncoderState::WritingHeader(header);
                }
                WsEncoderState::WritingHeader(mut header) => {
                    let iovec: [IoSlice; 2] = if this.buffer_for_split_control_frames.is_empty() {
                        [IoSlice::new(&header), IoSlice::new(&buf[..])]
                    } else {
                        [
                            IoSlice::new(&header),
                            IoSlice::new(&this.buffer_for_split_control_frames),
                        ]
                    };
                    match tokio::io::AsyncWrite::poll_write_vectored(
                        this.inner.writer.as_mut(),
                        cx,
                        &iovec,
                    ) {
                        Poll::Ready(Ok(n)) => {
                            let written_header_n = n.min(header.len());
                            let extra_n = n - written_header_n;
                            let remaining_header = header.split_off(written_header_n);
                            if remaining_header.is_empty() {
                                if this.buffer_for_split_control_frames.is_empty() {
                                    this.state = WsEncoderState::WritingData(extra_n);
                                } else {
                                    let _ = this.buffer_for_split_control_frames.split_to(extra_n);
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
                WsEncoderState::WritingData(offset) if offset == buf.len() => {
                    if this.terminate_pending {
                        this.state = WsEncoderState::Terminating;
                    } else if this.flush_pending {
                        this.state = WsEncoderState::Flushing;
                    } else {
                        this.state = WsEncoderState::PacketCompleted;
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
                            this.state = WsEncoderState::WritingData(new_offset);
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
                    if this.shutdown_socket_on_eof {
                        match tokio::io::AsyncWrite::poll_shutdown(this.inner.writer.as_mut(), cx) {
                            Poll::Ready(Ok(())) => {
                                debug!("shutdown completed");
                                this.state = WsEncoderState::PacketCompleted;
                            }
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                            Poll::Pending => return Poll::Pending,
                        }
                    } else {
                        debug!("Not shutting down the socket for writing");
                        this.state = WsEncoderState::Flushing;
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

//@ Wrap downstream stream-orinted writer to make expose packet-orinted sink using WebSocket framing
fn ws_encoder(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamWrite>,
) -> RhResult<Handle<DatagramWrite>> {
    let span = debug_span!("ws_encoder");
    #[derive(serde::Deserialize)]
    struct WsEncoderOpts {
        //@ Use masking (i.e. client-style)
        masked: bool,
        #[serde(default)]
        no_flush_after_each_message: bool,

        //@ Do not emit ConnectionClose frame when shutting down
        #[serde(default)]
        no_close_frame: bool,

        //@ Shutdown downstream socket for writing when shutting down
        #[serde(default)]
        shutdown_socket_on_eof: bool,
    }
    let opts: WsEncoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");

    let rng = if opts.masked {
        let the_scenario = ctx.get_scenario()?;
        let prng = rand_pcg::Pcg64::from_rng(&mut *the_scenario.prng.lock().unwrap());
        Some(prng)
    } else {
        None
    };

    let x = WsEncoder::new(
        span.clone(),
        rng,
        !opts.no_flush_after_each_message,
        inner,
        !opts.no_close_frame,
        opts.shutdown_socket_on_eof,
    );
    let x = DatagramWrite { snk: Box::pin(x) };
    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

#[pin_project]
pub struct WsDecoder {
    span: Span,
    #[pin]
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
        let mut this = self.project();
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

        trace!("poll_read");
        let mut need_to_issue_inner_read = false;
        loop {
            let mut pending_exit_from_loop = false;
            let unprocessed_data_range: Range<usize> = {
                if *this.unprocessed_bytes > 0 || !need_to_issue_inner_read {
                    assert!(*this.unprocessed_bytes <= buf.len());

                    *this.offset..(*this.offset + *this.unprocessed_bytes)
                } else {
                    trace!("inner read");
                    let mut rb = ReadBuf::new(&mut buf[*this.offset..]);
                    match tokio::io::AsyncRead::poll_read(this.inner.as_mut(), cx, &mut rb) {
                        Poll::Ready(Ok(())) => {
                            *this.unprocessed_bytes = rb.filled().len();
                            if *this.unprocessed_bytes == 0 {
                                outflags |= BufferFlag::Eof;
                                outflags &= !BufferFlag::NonFinalChunk;
                                pending_exit_from_loop = true;
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    };
                    need_to_issue_inner_read = false;

                    *this.offset..(*this.offset + *this.unprocessed_bytes)
                }
            };
            trace!(range=?unprocessed_data_range.clone(), "data ready");

            let unprocessed_data: &mut [u8] = &mut buf[unprocessed_data_range.clone()];

            let ret = this.wd.add_data(unprocessed_data);

            trace!(?ret, "decoded");
            #[allow(irrefutable_let_patterns)]
            let Ok(ret) = ret
            else {
                invdata!()
            };

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
                    if *this.require_masked && frame_info.mask.is_none() {
                        warn!("Unmasked frame where masked expected");
                        invdata!();
                    }
                    if *this.require_unmasked && frame_info.mask.is_some() {
                        warn!("Masked frame where unmasked is expected");
                        invdata!();
                    }
                    fill_in_flags_based_on_opcode!(original_opcode);
                }
                Some(websocket_sans_io::WebsocketFrameEvent::PayloadChunk { original_opcode }) => {
                    fill_in_flags_based_on_opcode!(original_opcode);
                    assert!(outrange.is_none());
                    outrange = Some(*this.offset..(*this.offset + ret.consumed_bytes));
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

            *this.offset += ret.consumed_bytes;
            *this.unprocessed_bytes -= ret.consumed_bytes;
            if *this.unprocessed_bytes == 0 {
                *this.offset = 0;
                trace!("fully processed this read chunk")
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

//@ Wrap downstream stream-orinted reader to make expose packet-orinted source using WebSocket framing
fn ws_decoder(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamRead>,
) -> RhResult<Handle<DatagramRead>> {
    let span = debug_span!("ws_decoder");
    #[derive(serde::Deserialize)]
    struct WsDecoderOpts {
        //@ Require decoded frames to be masked (i.e. coming from a client)
        #[serde(default)]
        require_masked: bool,
        //@ Require decoded frames to be masked (i.e. coming from a server)
        #[serde(default)]
        require_unmasked: bool,
    }
    let opts: WsDecoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
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
