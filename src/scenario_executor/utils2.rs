use std::fmt::Display;

use bytes::BytesMut;
use rhai::NativeCallContext;
use tracing::{debug, Span};

use crate::scenario_executor::utils1::SimpleErr;

use super::{
    types::{BufferFlag, BufferFlags, Registry, SocketFd},
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
        fallback: Option<T>,
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

        if n != 1 && fallback.is_none() {
            return Err(ctx.err("Exactly one of `addr` or `fd` or `fd_named` must be specified"));
        }
        if fallback.is_some() && n > 1 {
            return Err(ctx.err("At most one of `bind` or `fd` or `fd_named` must be specified"));
        }

        Ok(if let Some(x) = addr {
            debug!(parent: span, addr=%x, "options parsed");
            AddressOrFd::Addr(x)
        } else if let Some(x) = fd {
            debug!(parent: span, fd=%x, "options parsed");
            AddressOrFd::Fd(x)
        } else if let Some(x) = named_fd {
            debug!(parent: span, named_fd=%x, "options parsed");
            AddressOrFd::NamedFd(x)
        } else if let Some(x) = fallback {
            debug!(parent: span, addr=%x, "options parsed");
            AddressOrFd::Addr(x)
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
            debug!(parent: span, addr=?path, r#abstract=r#abstract, "options parsed");
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

#[cfg(unix)]
impl SocketFd {
    pub fn as_i64(&self) -> i64 {
        use std::os::fd::AsRawFd;
        self.0.as_raw_fd() as i64
    }

    pub fn as_raw_fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self.0.as_raw_fd()
    }

    /// # Safety
    /// May be unsound. Soundness may depend on sanity of options supplied by end user.
    /// `SocketFd` may be used to rip file descriptor away from e.g. TcpStream for use with `dup2`.
    /// There is no code to check that it was not closed or to remove extra `TcpStream``.
    pub unsafe fn new(x: std::os::fd::RawFd) -> Self {
        if x as i64 == -1 {
            panic!("Invalid file descriptor in SocketFd::new");
        }
        Self(
            // # Safety
            // May be IO-unsafe, soundness may depend on sanity of options supplied by end user.
            unsafe { std::os::fd::BorrowedFd::borrow_raw(x) },
        )
    }

    /// # Safety
    /// Depends on other code (including end-user-supplied scenarios) not doing unreasonable things.
    /// Intended to aid flexibility of low-lowlevel hacks and tricks.
    pub unsafe fn from_i64(x: i64) -> Option<Self> {
        if x == -1 {
            None
        } else {
            Some(
                // # Safety
                // Depends on other code (including end-user-supplied scenarios) not doing unreasonable things
                unsafe { SocketFd::new(x as std::os::fd::RawFd) },
            )
        }
    }
}

pub trait SocketFdI64 {
    fn maybe_as_i64(&self) -> i64;
}

impl SocketFdI64 for Option<SocketFd> {
    fn maybe_as_i64(&self) -> i64 {
        match self {
            Some(x) => x.as_i64(),
            None => -1,
        }
    }
}

#[cfg(not(unix))]
impl SocketFd {
    pub fn as_i64(&self) -> i64 {
        -1
    }

    pub unsafe fn from_i64(_x: i64) -> Option<Self> {
        None
    }
}

pub trait PollSemaphoreNew2 {
    fn new2(permits: usize) -> Self;
}

impl PollSemaphoreNew2 for tokio_util::sync::PollSemaphore {
    fn new2(permits: usize) -> Self {
        tokio_util::sync::PollSemaphore::new(std::sync::Arc::new(tokio::sync::Semaphore::new(
            permits,
        )))
    }
}
