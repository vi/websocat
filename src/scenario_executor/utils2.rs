use std::fmt::Display;

use bytes::BytesMut;
use rhai::NativeCallContext;
use tracing::{debug, Span};

use crate::scenario_executor::utils1::SimpleErr;

use super::{
    types::{BufferFlag, BufferFlags, Registry},
    utils1::{IsControlFrame, RhResult},
};

/// Assembles datagram from multiple sequention concatenated parts
pub struct Defragmenter {
    /// Presense of this indicates there is some incomplete unsent (or not fully sent) data
    incomplete_outgoing_datagram_buffer: Option<BytesMut>,

    /// `true` means that we have assembled the datagram fully, but failed to deliver it yet.
    incomplete_outgoing_datagram_buffer_complete: bool,

    max_size: usize,
}

pub enum DefragmenterAddChunkResult<'a> {
    DontSendYet,
    /// Refers either to `add_chunk`'s input or to internal buffer.
    Continunous(&'a [u8]),
    /// Attempted to exceede the max_size limit.
    /// Returned buffer is remembered data (not including new content supplied to `add_chunk`)
    SizeLimitExceeded(&'a [u8]),
}

impl Defragmenter {
    pub fn new(max_size: usize) -> Defragmenter {
        Defragmenter {
            incomplete_outgoing_datagram_buffer: None,
            incomplete_outgoing_datagram_buffer_complete: false,
            max_size,
        }
    }

    pub fn add_chunk<'a>(
        &'a mut self,
        buf: &'a mut [u8],
        flags: BufferFlags,
    ) -> DefragmenterAddChunkResult<'a> {
        let this = self;

        // control packets are typically for WebSocket things like pings, so let's ignore them
        if flags.is_control() {
            return DefragmenterAddChunkResult::DontSendYet;
        }

        if flags.contains(BufferFlag::NonFinalChunk) {
            let internal_buffer = this
                .incomplete_outgoing_datagram_buffer
                .get_or_insert_with(Default::default);
            if buf.len() > this.max_size || internal_buffer.len() + buf.len() > this.max_size {
                return DefragmenterAddChunkResult::SizeLimitExceeded(&internal_buffer[..]);
            }
            internal_buffer.extend_from_slice(buf);
            return DefragmenterAddChunkResult::DontSendYet;
        }
        let data: &[u8] = if let Some(ref mut x) = this.incomplete_outgoing_datagram_buffer {
            if !this.incomplete_outgoing_datagram_buffer_complete {
                x.extend_from_slice(buf);
                this.incomplete_outgoing_datagram_buffer_complete = true;
            }
            &x[..]
        } else {
            if buf.len() > this.max_size {
                return DefragmenterAddChunkResult::SizeLimitExceeded(b"");
            }
            buf
        };
        DefragmenterAddChunkResult::Continunous(data)
    }

    pub fn clear(&mut self) {
        self.incomplete_outgoing_datagram_buffer_complete = false;
        self.incomplete_outgoing_datagram_buffer = None;
    }
}

impl Registry {
    fn get_entry<T>(
        &self,
        id: &str,
        f: impl FnOnce(&flume::Sender<rhai::Dynamic>, &flume::Receiver<rhai::Dynamic>) -> T,
    ) -> T {
        let mut s = self.0.lock().unwrap();
        let q = if s.contains_key(id) {
            s.get_mut(id).unwrap()
        } else {
            s.entry(id.to_owned()).or_insert(flume::bounded(0))
        };
        f(&q.0, &q.1)
    }

    pub fn get_sender(&self, id: &str) -> flume::Sender<rhai::Dynamic> {
        self.get_entry(id, |x, _| x.clone())
    }

    pub fn get_receiver(&self, id: &str) -> flume::Receiver<rhai::Dynamic> {
        self.get_entry(id, |_, x| x.clone())
    }
}

pub enum AddressOrFd<T> {
    Addr(T),
    Fd(i32),
    NamedFd(String),
}

impl<T: Display> AddressOrFd<T> {
    pub fn interpret(
        ctx: &NativeCallContext,
        span: &Span,
        addr: Option<T>,
        fd: Option<i32>,
        named_fd: Option<String>,
    ) -> RhResult<Self> {
        let mut n = 0;
        if addr.is_some() {
            n += 1
        }
        if fd.is_some() {
            n += 1
        }
        if named_fd.is_some() {
            n += 1
        }

        if n != 1 {
            return Err(ctx.err("Exactly one of `addr` or `fd` or `fd_named` must be specified"));
        }

        Ok(if let Some(x) = addr {
            debug!(parent: span, listen_addr=%x, "options parsed");
            AddressOrFd::Addr(x)
        } else if let Some(x) = fd {
            debug!(parent: span, fd=%x, "options parsed");
            AddressOrFd::Fd(x)
        } else if let Some(x) = named_fd {
            debug!(parent: span, named_fd=%x, "options parsed");
            AddressOrFd::NamedFd(x)
        } else {
            unreachable!()
        })
    }
}

impl AddressOrFd<std::ffi::OsString> {
    pub fn interpret_path(
        ctx: &NativeCallContext,
        span: &Span,
        path: std::ffi::OsString,
        fd: Option<i32>,
        named_fd: Option<String>,
        r#abstract: bool,
    ) -> RhResult<Self> {
        let mut n = 0;
        if !path.is_empty() {
            n += 1
        }
        if fd.is_some() {
            n += 1
        }
        if named_fd.is_some() {
            n += 1
        }

        if n != 1 {
            return Err(ctx.err("Exactly one of `addr` or `fd` or `fd_named` must be specified"));
        }

        Ok(if !path.is_empty() {
            debug!(parent: span, listen_addr=?path, r#abstract=r#abstract, "options parsed");
            AddressOrFd::Addr(path)
        } else if let Some(x) = fd {
            debug!(parent: span, fd=%x, "options parsed");
            AddressOrFd::Fd(x)
        } else if let Some(x) = named_fd {
            debug!(parent: span, named_fd=%x, "options parsed");
            AddressOrFd::NamedFd(x)
        } else {
            unreachable!()
        })
    }
}

impl<T> AddressOrFd<T> {
    pub fn addr(&self) -> Option<&T> {
        match self {
            AddressOrFd::Addr(x) => Some(x),
            _ => None,
        }
    }
}
