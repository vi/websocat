use std::{io::IoSlice, pin::Pin, sync::Arc, task::Poll};

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

struct ReadLengthprefixedChunks {
    inner: StreamRead,
    separator: u8,
    separator_n: usize,

    /// Bytes read from the inner stream, but not yet scanned
    unprocessed_bytes: usize,
    /// Bytes that match `self.separator`, but not yet returned upstream as a part of a slice
    separator_bytes_in_a_row: usize,
    /// Offset. Relevant when one inner read leads to multiple returned frames.
    offset: usize,
}

impl ReadLengthprefixedChunks {
    #[allow(unused)]
    pub fn new(inner: StreamRead, separator: u8, separator_n: usize) -> Self {
        Self {
            inner,
            separator,
            separator_n,
            unprocessed_bytes: 0,
            separator_bytes_in_a_row: 0,
            offset: 0,
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
        assert!(this.separator_n < buf.len());

        if this.unprocessed_bytes == 0 {
            assert!(this.separator_bytes_in_a_row < this.separator_n);

            // if there is unfinished possible separator in the middle,
            // prepend it to the buffer
            this.offset = this.separator_bytes_in_a_row;
            buf[0..this.offset].fill(this.separator);

            let sr = Pin::new(&mut this.inner);
            let mut rb = ReadBuf::new(&mut buf[this.offset..]);

            match tokio::io::AsyncRead::poll_read(sr, cx, &mut rb) {
                Poll::Ready(Ok(())) => {
                    this.unprocessed_bytes = rb.filled().len();
                    if this.unprocessed_bytes == 0 {
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: BufferFlag::Eof.into(),
                            buffer_subset: 0..0,
                        }));
                    }
                    // wind back to the beginning of the buffer
                    // where we have put in-middle-of-possible-separator debt
                    this.unprocessed_bytes += this.separator_bytes_in_a_row;
                    this.offset = 0;
                    // we have turned those bytes into actual separator characters in the buffer
                    this.separator_bytes_in_a_row = 0;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        let chunk_start = this.offset;
        let mut chunk_end = this.offset;

        for &b in buf[this.offset..(this.offset + this.unprocessed_bytes)].iter() {
            this.unprocessed_bytes -= 1;
            this.offset += 1;
            if b == this.separator {
                this.separator_bytes_in_a_row += 1;
                if this.separator_bytes_in_a_row == this.separator_n {
                    let ret = Poll::Ready(Ok(PacketReadResult {
                        flags: BufferFlag::Text.into(),
                        buffer_subset: chunk_start..chunk_end,
                    }));
                    this.separator_bytes_in_a_row = 0;
                    return ret;
                }
            } else {
                chunk_end += 1;
                chunk_end += this.separator_bytes_in_a_row;
                this.separator_bytes_in_a_row = 0;
            }
        }

        Poll::Ready(Ok(PacketReadResult {
            flags: BufferFlag::Text | BufferFlag::NonFinalChunk,
            buffer_subset: chunk_start..chunk_end,
        }))
    }
}

struct WriteLengthprefixedChunks {
    w: StreamWrite,
    degragmenter: Defragmenter,
    /// If None then header is not yet generated. If Some(empty) then the header is already written.
    header: Option<ArrayVec<[u8; 8]>>,
    /// Used as a cursor when writing the body of the message
    debt: usize,
    opts: Arc<OptsShared>,
}

impl WriteLengthprefixedChunks {
    pub fn new(inner: StreamWrite, opts: Arc<OptsShared>) -> Self {
        if opts.controls.is_some() {
            todo!()
        }
        Self {
            w: inner,
            degragmenter: Defragmenter::new(opts.max_message_size),
            header: None,
            debt: 0,
            opts,
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

        let data: &[u8] = if p.opts.continuations .is_some() {
            if flags.is_control() {
                return Poll::Ready(Ok(()));
            }
            buf_
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
        };

        if data.len() as u64 > p.opts.length_mask {
            warn!("Message length is larger than `lengthprefixed:` header could handle. Closing this session.");
            return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
        }

        if p.header.is_none() {
            let mut h: u64 = (data.len() as u64) & p.opts.length_mask;

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
            todo!();
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
