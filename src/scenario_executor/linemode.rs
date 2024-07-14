use std::{io::IoSlice, pin::Pin, task::Poll};

use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::io::ReadBuf;
use tracing::debug;

use crate::scenario_executor::utils::{ExtractHandleOrFail, HandleExt, SimpleErr};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite, StreamRead, StreamSocket, StreamWrite,
    },
    utils::RhResult,
};

struct ReadLineChunks {
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

impl ReadLineChunks {
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

impl PacketRead for ReadLineChunks {
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

struct WriteLineChunks {
    w: StreamWrite,
    separator: Vec<u8>,
    intramessage_separator_substitution: Option<u8>,
    buffer_offset: usize,
    separator_offset: usize,

    /// message_has_currently_this_number_of_repeated_separator_bytes_at_the_last_chunk_end
    mhctnorsbatlce: usize,
    nonfirst_chunk: bool,
    /// Separator bytes redacted from the chunk that may needed to be reinserted at the beginning of the next chunk
    indebted_separator_bytes: usize,
    chunk_already_processed: bool,
    debt: Option<Vec<u8>>,
    trim_bytes_from_start: usize,
    trim_bytes_from_end: usize,
}

impl WriteLineChunks {
    pub fn new(inner: StreamWrite, separator: u8, separator_n: usize, subst: Option<u8>) -> Self {
        assert!(separator_n>0);
        Self {
            w: inner,
            separator: vec![separator; separator_n],
            buffer_offset: 0,
            separator_offset: 0,
            intramessage_separator_substitution: subst,
            mhctnorsbatlce: 0,
            nonfirst_chunk: false,
            indebted_separator_bytes: 0,
            chunk_already_processed: false,
            debt: None,
            trim_bytes_from_start: 0,
            trim_bytes_from_end: 0,
        }
    }
}

impl PacketWrite for WriteLineChunks {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let required_separator_len = if flags.contains(BufferFlag::NonFinalChunk) {
            0
        } else if flags.contains(BufferFlag::Eof) {
            0
        } else {
            this.separator.len()
        };

        if let (Some(subst), false) = (this.intramessage_separator_substitution, this.chunk_already_processed) {
            let sb = this.separator[0];

            this.trim_bytes_from_start = 0;
            this.trim_bytes_from_end = 0;

            if !this.nonfirst_chunk {
                while buf[this.trim_bytes_from_start..].first() == Some(&sb) {
                    this.trim_bytes_from_start+=1;
                }
                if !buf[this.trim_bytes_from_start..].is_empty() {
                    this.nonfirst_chunk = true;
                }
            }

            let mut there_is_nonseparator_byte = false;
            for x in buf.iter_mut() {
                if *x == sb {
                    this.mhctnorsbatlce += 1;
                    if this.mhctnorsbatlce >= this.separator.len() {
                        *x = subst;
                        there_is_nonseparator_byte = true;
                        this.mhctnorsbatlce=0;
                    }
                } else {
                    this.mhctnorsbatlce = 0;
                    there_is_nonseparator_byte = true;
                }
            }

            if there_is_nonseparator_byte && this.indebted_separator_bytes > 0 {
                this.debt = Some(vec![sb; this.indebted_separator_bytes]);
                this.indebted_separator_bytes = 0;
            }

            while buf[this.trim_bytes_from_start..(buf.len()-this.trim_bytes_from_end)].last() == Some(&sb) {
                this.indebted_separator_bytes += 1;
                this.trim_bytes_from_end+=1;
            }
            assert!(this.indebted_separator_bytes < this.separator.len());

            if !flags.contains(BufferFlag::NonFinalChunk) {
                this.nonfirst_chunk = false;
                this.indebted_separator_bytes = 0;
            }

            this.chunk_already_processed = true;
        }

        loop {
            if let Some(ref debt) = this.debt {
                match tokio::io::AsyncWrite::poll_write(Pin::new(&mut this.w.writer), cx, &debt) {
                    Poll::Ready(Ok(n)) => {
                        if n >= debt.len() {
                            this.debt = None;
                        } else {
                            this.debt = Some(debt[n..].to_vec());
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            }
            let buf = &buf[this.trim_bytes_from_start..(buf.len()-this.trim_bytes_from_end)];
            assert!(buf.len() >= this.buffer_offset);
            let buf_chunk = &buf[this.buffer_offset..];
            if buf_chunk.is_empty() && this.separator_offset == required_separator_len {
                if !flags.contains(BufferFlag::NonFinalChunk) {
                    match tokio::io::AsyncWrite::poll_flush(Pin::new(&mut this.w.writer), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                if flags.contains(BufferFlag::Eof) {
                    match tokio::io::AsyncWrite::poll_shutdown(Pin::new(&mut this.w.writer), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                this.buffer_offset = 0;
                this.separator_offset = 0;
                break;
            }
            let bufs : [IoSlice; 2] = [
                IoSlice::new(buf_chunk),
                IoSlice::new(&this.separator[this.separator_offset..required_separator_len]),
            ];
            match tokio::io::AsyncWrite::poll_write_vectored(Pin::new(&mut this.w.writer), cx, &bufs) {
                Poll::Ready(Ok(mut n)) => {
                    let n_from_chunk = n.min(buf_chunk.len());
                    this.buffer_offset += n_from_chunk;
                    n -= n_from_chunk;
                    this.separator_offset += n;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        this.chunk_already_processed = false;
        return Poll::Ready(Ok(()));
    }
}

fn line_chunks(
    ctx: NativeCallContext,
    opts: Dynamic,
    x: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let x = ctx.lutbar(x)?;

    #[derive(serde::Deserialize)]
    struct LineChunksOpts {
        //@ Use this byte as a separator. Defaults to 10 (\n).
        separator: Option<u8>,

        //@ Use this number of repetitions of the specified byte to consider it as a separator. Defaults to 1.
        separator_n: Option<usize>,

        //@ When framing messages, look for byte sequences within the message that may alias with
        //@ the separator and substitute last byte of such pseudo-separators with this byte value.
        //@
        //@ If active, leading and trailing separator bytes are also removed from the datagrams
        substitute: Option<u8>,
    }
    let opts: LineChunksOpts = rhai::serde::from_dynamic(&opts)?;

    let separator = opts.separator.unwrap_or(b'\n');
    let separator_n = opts.separator_n.unwrap_or(1);
    if separator_n == 0 {
        return Err(ctx.err("Zero separator_n specified"));
    }

    debug!(inner=?x, "line_chunks: parsed opts");

    let mut wrapped = DatagramSocket {
        read: None,
        write: None,
        close: x.close,
    };

    if let Some(r) = x.read {
        wrapped.read = Some(DatagramRead {
            src: Box::pin(ReadLineChunks::new(r, separator, separator_n)),
        })
    }

    if let Some(w) = x.write {
        wrapped.write = Some(DatagramWrite {
            snk: Box::pin(WriteLineChunks::new(w, separator, separator_n, opts.substitute)),
        })
    }

    debug!(?wrapped, "line_chunks");
    Ok(Some(wrapped).wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("line_chunks", line_chunks);
}
