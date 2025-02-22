use std::{
    io::IoSlice,
    pin::Pin,
    sync::Arc,
    task::{ready, Poll},
};

use bytes::BytesMut;
use rhai::{Dynamic, Engine, NativeCallContext};
use tinyvec::ArrayVec;
use tokio::io::ReadBuf;
use tracing::{debug, warn};

use crate::scenario_executor::{
    trivials2::{ReadStreamChunks, WriteStreamChunks},
    utils1::{ExtractHandleOrFail, HandleExt},
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite, StreamRead, StreamSocket, StreamWrite,
    },
    utils1::{IsControlFrame, RhResult},
    utils2::{Defragmenter, DefragmenterAddChunkResult},
    MAX_CONTROL_MESSAGE_LEN,
};

#[derive(Debug)]
struct OptsShared {
    length_mask: u64,
    nbytes: usize,
    max_message_size: usize,
    little_endian: bool,
    continuations: Option<u64>,
    controls: Option<u64>,
    tag_text: Option<u64>,
}

enum ReadLengthprefixedChunksState {
    ReadingHeader(ArrayVec<[u8; 8]>),
    ReadingControlFrameOpcode { nonfinal: bool, remaining: u64 },
    StreamingData { flags: BufferFlags, remaining: u64 },
}

struct ReadLengthprefixedChunks {
    inner: StreamRead,
    opts: Arc<OptsShared>,
    state: ReadLengthprefixedChunksState,
}

impl ReadLengthprefixedChunks {
    #[allow(unused)]
    pub fn new(inner: StreamRead, opts: Arc<OptsShared>) -> Self {
        Self {
            inner,
            opts,
            state: ReadLengthprefixedChunksState::ReadingHeader(Default::default()),
        }
    }
}

impl PacketRead for ReadLengthprefixedChunks {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();

        let mut tmpbuf = [0; 8];

        loop {
            match &mut this.state {
                ReadLengthprefixedChunksState::ReadingHeader(array_vec) => {
                    let required_len = this.opts.nbytes;

                    if array_vec.len() == required_len {
                        let mut h = ArrayVec::from_array_empty([0u8; 8]);
                        h.set_len(8 - this.opts.nbytes);

                        if this.opts.little_endian {
                            h.extend(array_vec.iter().rev().copied());
                        } else {
                            h.extend_from_slice(array_vec);
                        }

                        let h = u64::from_be_bytes(h.into_inner());

                        let payload_len = h & this.opts.length_mask;

                        let cont = if let Some(x) = this.opts.continuations {
                            h & x != 0
                        } else {
                            false
                        };

                        let ctrl = if let Some(x) = this.opts.controls {
                            h & x != 0
                        } else {
                            false
                        };

                        let txt = if let Some(x) = this.opts.tag_text {
                            h & x != 0
                        } else {
                            false
                        };

                        let mut flags = BufferFlags::default();

                        if cont {
                            flags |= BufferFlag::NonFinalChunk;
                        }
                        if txt {
                            flags |= BufferFlag::Text;
                        }

                        this.state = if ctrl {
                            if payload_len < 1 {
                                warn!("Invalid payload length of a lengthprefixed: control frame");
                                return Poll::Ready(Ok(PacketReadResult {
                                    flags: BufferFlag::Eof.into(),
                                    buffer_subset: 0..0,
                                }));
                            }
                            ReadLengthprefixedChunksState::ReadingControlFrameOpcode {
                                nonfinal: cont,
                                remaining: payload_len,
                            }
                        } else {
                            ReadLengthprefixedChunksState::StreamingData {
                                flags,
                                remaining: payload_len,
                            }
                        };
                        continue;
                    }

                    let missing_len = required_len - array_vec.len();

                    let mut rb = ReadBuf::new(&mut tmpbuf[..missing_len]);

                    ready!(tokio::io::AsyncRead::poll_read(
                        Pin::new(&mut this.inner),
                        cx,
                        &mut rb
                    ))?;

                    if rb.filled().is_empty() {
                        if !array_vec.is_empty() {
                            warn!("Trimmed input data of lengthprefixed: overlay")
                        }
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: BufferFlag::Eof.into(),
                            buffer_subset: 0..0,
                        }));
                    }

                    array_vec.extend_from_slice(rb.filled());
                }
                ReadLengthprefixedChunksState::ReadingControlFrameOpcode {
                    remaining,
                    nonfinal,
                } => {
                    let mut opcode = [0];
                    let mut rb = ReadBuf::new(&mut opcode[..]);

                    ready!(tokio::io::AsyncRead::poll_read(
                        Pin::new(&mut this.inner),
                        cx,
                        &mut rb
                    ))?;

                    if rb.filled().is_empty() {
                        warn!("Trimmed input data of lengthprefixed: overlay");
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: BufferFlag::Eof.into(),
                            buffer_subset: 0..0,
                        }));
                    }

                    assert_eq!(rb.filled().len(), 1);

                    let mut flags = BufferFlags::default();
                    match opcode[0] {
                        0x08 => {
                            flags |= BufferFlag::Eof;
                        }
                        0x09 => {
                            flags |= BufferFlag::Ping;
                        }
                        0x0A => {
                            flags |= BufferFlag::Pong;
                        }
                        _ => {
                            warn!("Invalid lengthprefixed: opcode {}", opcode[0]);
                            return Poll::Ready(Ok(PacketReadResult {
                                flags: BufferFlag::Eof.into(),
                                buffer_subset: 0..0,
                            }));
                        }
                    }
                    if *nonfinal {
                        flags |= BufferFlag::NonFinalChunk;
                    }
                    this.state = ReadLengthprefixedChunksState::StreamingData {
                        flags,
                        remaining: *remaining - 1,
                    };
                }
                ReadLengthprefixedChunksState::StreamingData { flags, remaining } => {
                    let mut flags = *flags;

                    if *remaining == 0 {
                        this.state =
                            ReadLengthprefixedChunksState::ReadingHeader(Default::default());

                        return Poll::Ready(Ok(PacketReadResult {
                            flags,
                            buffer_subset: 0..0,
                        }));
                    }

                    let mut limit = buf.len();
                    if limit as u64 > *remaining {
                        limit = *remaining as usize;
                    }

                    let mut rb = ReadBuf::new(&mut buf[..limit]);

                    ready!(tokio::io::AsyncRead::poll_read(
                        Pin::new(&mut this.inner),
                        cx,
                        &mut rb
                    ))?;

                    if rb.filled().is_empty() {
                        warn!("Trimmed input data of lengthprefixed: overlay");
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: BufferFlag::Eof.into(),
                            buffer_subset: 0..0,
                        }));
                    }

                    *remaining -= rb.filled().len() as u64;

                    if *remaining != 0 {
                        flags |= BufferFlag::NonFinalChunk;
                    } else {
                        this.state =
                            ReadLengthprefixedChunksState::ReadingHeader(Default::default());
                    }

                    return Poll::Ready(Ok(PacketReadResult {
                        flags,
                        buffer_subset: 0..(rb.filled().len()),
                    }));
                }
            }
        }
    }
}

struct WriteLengthprefixedChunks {
    w: StreamWrite,
    degragmenter: Defragmenter,
    /// If None then header is not yet generated. If Some(empty) then the header is already written.
    header: Option<ArrayVec<[u8; 9]>>,
    /// Used as a cursor when writing the body of the message
    debt: usize,
    opts: Arc<OptsShared>,
    buffer_for_split_control_frames: BytesMut,
}

impl WriteLengthprefixedChunks {
    pub fn new(inner: StreamWrite, opts: Arc<OptsShared>) -> Self {
        Self {
            w: inner,
            degragmenter: Defragmenter::new(opts.max_message_size),
            header: None,
            debt: 0,
            opts,
            buffer_for_split_control_frames: Default::default(),
        }
    }
}

impl PacketWrite for WriteLengthprefixedChunks {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf_: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let p = self.get_mut();
        let sw: &mut StreamWrite = &mut p.w;

        let data: &[u8] = if p.opts.continuations.is_some() {
            if flags.is_control() && p.opts.controls.is_none() {
                return Poll::Ready(Ok(()));
            }
            buf_
        } else {
            if flags.is_control() && p.opts.controls.is_some() {
                if flags.contains(BufferFlag::NonFinalChunk) {
                    p.buffer_for_split_control_frames.extend_from_slice(buf_);
                    return Poll::Ready(Ok(()));
                }
                if !p.buffer_for_split_control_frames.is_empty() {
                    if p.buffer_for_split_control_frames.len() > MAX_CONTROL_MESSAGE_LEN {
                        warn!("Excessive control message size");
                        return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
                    }

                    p.buffer_for_split_control_frames.extend_from_slice(buf_);
                    &p.buffer_for_split_control_frames[..]
                } else {
                    buf_
                }
            } else {
                match p.degragmenter.add_chunk(buf_, flags) {
                    DefragmenterAddChunkResult::DontSendYet => {
                        return Poll::Ready(Ok(()));
                    }
                    DefragmenterAddChunkResult::Continunous(x) => x,
                    DefragmenterAddChunkResult::SizeLimitExceeded(_x) => {
                        warn!("Exceeded maximum allowed outgoing datagram size. Closing this session.");
                        return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
                    }
                }
            }
        };

        let mut payloadlen = data.len() as u64;
        if flags.is_control() {
            payloadlen += 1;
        }

        if payloadlen > p.opts.length_mask {
            warn!("Message length is larger than `lengthprefixed:` header could handle. Closing this session.");
            return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
        }

        if p.header.is_none() {
            let mut h: u64 = payloadlen;

            if let Some(x) = p.opts.tag_text {
                if flags.contains(BufferFlag::Text) {
                    h |= x;
                }
            }
            if let Some(x) = p.opts.continuations {
                if flags.contains(BufferFlag::NonFinalChunk) {
                    h |= x;
                }
            }
            if let Some(x) = p.opts.controls {
                if flags.is_control() {
                    h |= x;
                }
            }

            let h = h.to_be_bytes();

            let mut hc = ArrayVec::new();
            let nb = p.opts.nbytes;

            if p.opts.little_endian {
                for i in 0..nb {
                    hc.push(h[7 - i]);
                }
            } else {
                for i in 0..nb {
                    hc.push(h[(8 - nb) + i]);
                }
            }

            if flags.is_control() {
                if flags.contains(BufferFlag::Eof) {
                    hc.push(8);
                } else if flags.contains(BufferFlag::Ping) {
                    hc.push(9);
                } else if flags.contains(BufferFlag::Pong) {
                    hc.push(10);
                } else {
                    hc.push(0xFF);
                }
            }

            p.header = Some(hc)
        }

        let Some(ref mut header) = p.header else {
            unreachable!()
        };

        loop {
            assert!(data.len() >= p.debt);
            let header_chunk = &header[..];
            let buf_chunk = &data[p.debt..];

            if buf_chunk.is_empty() && header_chunk.is_empty() {
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
                p.debt = 0;
                p.header = None;
                p.degragmenter.clear();
                p.buffer_for_split_control_frames.clear();
                break;
            }

            let bufs = [IoSlice::new(header_chunk), IoSlice::new(buf_chunk)];
            match sw.writer.as_mut().poll_write_vectored(cx, &bufs) {
                Poll::Ready(Ok(mut n)) => {
                    if header.len() > 0 {
                        let x = n.min(header.len());
                        *header = header.split_off(x);
                        n -= x;
                    }
                    p.debt += n;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        return Poll::Ready(Ok(()));
    }
}

//@ Convert downstream stream socket into upstream packet socket using a byte separator
//@
//@ If you want just source or sink conversion part, create incomplete socket, use this function, then extract the needed part from resulting incomplete socket.
fn length_prefixed_chunks(
    ctx: NativeCallContext,
    opts: Dynamic,
    x: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let x = ctx.lutbar(x)?;

    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Maximum message length that can be encoded in header, power of two minus one
        length_mask: u64,

        //@ Number of bytes in header field
        nbytes: usize,

        //@ Maximum size of a message that can be encoded, unless `continuations` is set to true. Does not affect decoded messages.
        max_message_size: usize,

        //@ Encode header as a little-endian number instead of big endian
        #[serde(default)]
        little_endian: bool,

        //@ Inhibit adding header to data transferred in read direction, pass byte chunks unmodifed
        #[serde(default)]
        skip_read_direction: bool,

        //@ Inhibit adding header to data transferred in read direction, pass byte chunks unmodifed
        #[serde(default)]
        skip_write_direction: bool,

        //@ Do not defragment written messages,.write WebSocket frames instead of messages (and `or` specified number into the header).
        continuations: Option<u64>,

        //@ Also write pings, pongs and CloseFrame messages, setting specified bit (pre-shifted) in header and prepending opcode in condent.
        //@ Length would include this prepended byte.
        //@
        //@ Affects read direction as well, allowing manually triggering WebSocket control messages.
        controls: Option<u64>,

        //@ Set specified pre-shifted bit in header when dealing with text WebSocket messages.
        //@ Note that with continuations, messages can be split into fragments in middle of a UTF-8 characters.
        tag_text: Option<u64>,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    debug!(inner=?x, "length_prefixed_chunks: parsed opts");

    let optss = Arc::new(OptsShared {
        length_mask: opts.length_mask,
        nbytes: opts.nbytes,
        max_message_size: opts.max_message_size,
        little_endian: opts.little_endian,
        continuations: opts.continuations,
        controls: opts.controls,
        tag_text: opts.tag_text,
    });

    let mut wrapped = DatagramSocket {
        read: None,
        write: None,
        close: x.close,
    };

    if let Some(r) = x.read {
        if opts.skip_read_direction {
            wrapped.read = Some(DatagramRead {
                src: Box::pin(ReadStreamChunks(r)),
            })
        } else {
            wrapped.read = Some(DatagramRead {
                src: Box::pin(ReadLengthprefixedChunks::new(r, optss.clone())),
            })
        }
    }

    if let Some(w) = x.write {
        if opts.skip_write_direction {
            wrapped.write = Some(DatagramWrite {
                snk: Box::pin(WriteStreamChunks { w, debt: 0 }),
            })
        } else {
            wrapped.write = Some(DatagramWrite {
                snk: Box::pin(WriteLengthprefixedChunks::new(w, optss)),
            })
        }
    }

    debug!(?wrapped, "length_prefixed_chunks");
    Ok(Some(wrapped).wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("length_prefixed_chunks", length_prefixed_chunks);
}
