use std::{
    ffi::{c_void, OsString},
    future::Future,
    io::{ErrorKind, IoSlice},
    os::fd::{AsRawFd, OwnedFd, RawFd},
    pin::Pin,
    sync::Arc,
    task::{ready, Poll},
    time::Duration,
};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite, SocketFd, StreamRead, StreamWrite},
    utils1::{HandleExt, SimpleErr, TaskHandleExt2},
    utils2::AddressOrFd,
};
use filedesc::FileDesc;
use futures::FutureExt;
use libc::{fcntl, read, F_GETFL, F_SETFL, O_NONBLOCK};
use nix::sys::uio::writev;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::io::{unix::AsyncFd, AsyncRead, AsyncWrite};
use tracing::{debug, debug_span, error, trace, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, Task},
};

use super::{
    types::{BufferFlag, BufferFlags, PacketRead, PacketReadResult, PacketWrite, StreamSocket},
    utils1::{RhResult, SignalOnDrop},
    utils2::{Defragmenter, DefragmenterAddChunkResult},
};
use clap_lex::OsStrExt;

use super::unix1::{
    abstractify, listen_from_fd, listen_from_fd_named, maybe_chmod, ListenFromFdType,
};

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
#[derive(Clone)]
struct SeqpacketSendAdapter {
    s: Arc<(tokio_seqpacket::UnixSeqpacket, SignalOnDrop)>,
    text: bool,
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
struct SeqpacketRecvAdapter {
    s: Arc<(tokio_seqpacket::UnixSeqpacket, SignalOnDrop)>,
    degragmenter: Defragmenter,
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
impl PacketRead for SeqpacketSendAdapter {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let size = std::task::ready!(self.s.0.poll_recv(cx, buf))?;

        let mut flags = if self.text {
            BufferFlag::Text.into()
        } else {
            BufferFlags::default()
        };

        // FIXME: discriminate zero-length datagrams from EOFs

        if size == 0 {
            flags |= BufferFlag::Eof;
        }

        Poll::Ready(Ok(PacketReadResult {
            flags,
            buffer_subset: 0..size,
        }))
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
impl PacketWrite for SeqpacketRecvAdapter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if flags.contains(BufferFlag::Eof) {
            this.s.0.shutdown(std::net::Shutdown::Write)?;
            return Poll::Ready(Ok(()));
        }

        let data: &[u8] = match this.degragmenter.add_chunk(buf, flags) {
            DefragmenterAddChunkResult::DontSendYet => {
                return Poll::Ready(Ok(()));
            }
            DefragmenterAddChunkResult::Continunous(x) => x,
            DefragmenterAddChunkResult::SizeLimitExceeded(_x) => {
                warn!("Exceeded maximum allowed outgoing datagram size. Closing this session.");
                return Poll::Ready(Err(ErrorKind::InvalidData.into()));
            }
        };

        let ret = this.s.0.poll_send(cx, data);

        match ready!(ret) {
            Ok(n) => {
                if n != data.len() {
                    warn!("short SEQPACKET send");
                }
            }
            Err(e) => {
                this.degragmenter.clear();
                return Poll::Ready(Err(e));
            }
        }

        this.degragmenter.clear();
        Poll::Ready(Ok(()))
    }
}

const fn default_max_send_datagram_size() -> usize {
    1048576
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
//@ Connect to a SOCK_SEQPACKET UNIX stream socket
fn connect_seqpacket(
    ctx: NativeCallContext,
    opts: Dynamic,
    mut path: OsString,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    use crate::scenario_executor::unix1::abstractify;

    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_seqpacket");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct ConnectSeqpacketOpts {
        //@ On Linux, connect to an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,

        //@ Mark received datagrams as text
        #[serde(default)]
        text: bool,

        //@ Default defragmenter buffer limit
        #[serde(default = "default_max_send_datagram_size")]
        max_send_datagram_size: usize,
    }
    let opts: ConnectSeqpacketOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, ?path, r#abstract=opts.r#abstract, "options parsed");

    if opts.r#abstract {
        abstractify(&mut path);
    } else if path.starts_with("@") {
        warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
    }

    Ok(async move {
        debug!("node started");
        let s = tokio_seqpacket::UnixSeqpacket::connect(path).await?;

        #[allow(unused_assignments)]
        let mut fd = None;
        #[cfg(unix)]
        {
            fd = Some(
                // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                unsafe { super::types::SocketFd::new(s.as_raw_fd()) },
            );
        }

        let s = Arc::new((s, SignalOnDrop::new_neutral()));
        let r = SeqpacketSendAdapter {
            s: s.clone(),
            text: opts.text,
        };
        let w = SeqpacketRecvAdapter {
            s,
            degragmenter: Defragmenter::new(opts.max_send_datagram_size),
        };
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = DatagramSocket {
            read: Some(DatagramRead { src: r }),
            write: Some(DatagramWrite { snk: w }),
            close: None,
            fd,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<DatagramSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
fn listen_seqpacket(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Path to a socket file to create, name of abstract address to use or empty string if `fd` is used.
    mut path: OsString,
    //@ Called once after the port is bound
    when_listening: FnPtr,
    //@ Call on each incoming connection
    on_accept: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("listen_seqpacket");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Inherited file descriptor to accept connections from
        fd: Option<i32>,

        //@ Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
        named_fd: Option<String>,

        //@ Skip socket type check when using `fd`.
        #[serde(default)]
        fd_force: bool,

        //@ On Linux, connect to an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,

        //@ Change filesystem mode (permissions) of the file after listening
        #[serde(default)]
        chmod: Option<u32>,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Mark received datagrams as text
        #[serde(default)]
        text: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,

        //@ Default defragmenter buffer limit
        #[serde(default = "default_max_send_datagram_size")]
        max_send_datagram_size: usize,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));

    let autospawn = opts.autospawn;
    let oneshot = opts.oneshot;

    if opts.r#abstract {
        abstractify(&mut path);
    } else if path.starts_with("@") {
        warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
    }
    let a =
        AddressOrFd::interpret_path(&ctx, &span, path, opts.fd, opts.named_fd, opts.r#abstract)?;

    Ok(async move {
        debug!("node started");

        let assertaddr = Some(ListenFromFdType::Seqpacket);
        let forceaddr = if opts.fd_force { assertaddr } else { None };
        let mut l = match &a {
            AddressOrFd::Addr(path) => tokio_seqpacket::UnixSeqpacketListener::bind(path)?,
            AddressOrFd::Fd(f) => {
                unsafe { listen_from_fd(*f, forceaddr, assertaddr) }?.unwrap_seqpacket()
            }
            AddressOrFd::NamedFd(f) => {
                unsafe { listen_from_fd_named(f, forceaddr, assertaddr) }?.unwrap_seqpacket()
            }
        };

        if let Some(path) = a.addr() {
            maybe_chmod(opts.chmod, path, || l.accept().now_or_never()).await?;
        }

        callback_and_continue::<()>(the_scenario.clone(), when_listening, ()).await;

        let mut i = 0;

        let mut drop_notification = None;

        loop {
            let the_scenario = the_scenario.clone();
            let on_accept = on_accept.clone();
            match l.accept().await {
                Ok(s) => {
                    let newspan = debug_span!("seqpacket_accept", i);

                    let fd = Some(
                        // Safety: may be unsound, as it exposes raw FDs to end-user-specifiable scenarios
                        unsafe { super::types::SocketFd::new(s.as_raw_fd()) },
                    );

                    i += 1;
                    let dropper = if oneshot {
                        let (a, b) = SignalOnDrop::new();
                        drop_notification = Some(b);
                        a
                    } else {
                        SignalOnDrop::new_neutral()
                    };
                    let s = Arc::new((s, dropper));
                    let r = SeqpacketSendAdapter {
                        s: s.clone(),
                        text: opts.text,
                    };
                    let w = SeqpacketRecvAdapter {
                        s,
                        degragmenter: Defragmenter::new(opts.max_send_datagram_size),
                    };
                    let (r, w) = (Box::pin(r), Box::pin(w));

                    let s = DatagramSocket {
                        read: Some(DatagramRead { src: r }),
                        write: Some(DatagramWrite { snk: w }),
                        close: None,
                        fd,
                    };

                    debug!(parent: &newspan, s=?s,"accepted");
                    let h = s.wrap();
                    if !autospawn {
                        callback_and_continue::<(Handle<DatagramSocket>,)>(
                            the_scenario,
                            on_accept,
                            (h,),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<DatagramSocket>,)>(
                                the_scenario,
                                on_accept,
                                (h,),
                            )
                            .instrument(newspan)
                            .await;
                        });
                    }
                }
                Err(e) => {
                    error!("Error from accept: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }

            if oneshot {
                debug!("Exiting SEQPACKET listener due to --oneshot mode");
                break;
            }
        }

        if let Some(dn) = drop_notification {
            debug!("Waiting for the sole accepted client to finish serving");
            let _ = dn.await;
            debug!("The sole accepted client finished");
        }

        Ok(())
    }
    .instrument(span)
    .wrap())
}

enum MyAsyncFdWay {
    Proper(AsyncFd<FileDesc>),
    Hacky {
        fd: OwnedFd,
        read_sleeper: Option<Pin<Box<tokio::time::Sleep>>>,
        write_sleeper: Option<Pin<Box<tokio::time::Sleep>>>,
    },
}
struct MyAsyncFd {
    inner: MyAsyncFdWay,
    need_to_restore_blocking_mode: bool,
}

impl Drop for MyAsyncFd {
    fn drop(&mut self) {
        if self.need_to_restore_blocking_mode {
            let x = match &self.inner {
                MyAsyncFdWay::Proper(async_fd) => async_fd.get_ref().as_raw_fd(),
                MyAsyncFdWay::Hacky { fd, .. } => fd.as_raw_fd(),
            };

            unsafe {
                let mut flags = fcntl(x, F_GETFL, 0);
                if flags == -1 {
                    return;
                }
                flags &= !O_NONBLOCK;
                if -1 == fcntl(x, F_SETFL, flags) {
                    return;
                }
            }
        }
    }
}

impl MyAsyncFd {
    /// # Safety
    ///
    /// Do not supply file descriptors that are not inherited from parent process, received from UNIX socket or exposed as raw FDs.
    unsafe fn new(fd: RawFd, force: bool) -> std::io::Result<Self> {
        let need_to_restore_blocking_mode = unsafe {
            let mut flags = fcntl(fd, F_GETFL, 0);
            if flags == -1 {
                error!("Failed to get flags of a user-specified file descriptor");
                return Err(ErrorKind::Other.into());
            }
            if flags & O_NONBLOCK != 0 {
                false
            } else {
                flags |= O_NONBLOCK;
                if -1 == fcntl(fd, F_SETFL, flags) {
                    error!("Failed to set flags of a user-specified file descriptor");
                    return Err(ErrorKind::Other.into());
                }
                true
            }
        };
        let inner = match AsyncFd::try_new(FileDesc::from_raw_fd(fd)) {
            Ok(x) => MyAsyncFdWay::Proper(x),
            Err(e) => {
                if force {
                    let (fdesc, e) = e.into_parts();
                    debug!("Failed to register FD {fd:?} for async events: {e}");

                    MyAsyncFdWay::Hacky {
                        fd: fdesc.into_fd(),
                        read_sleeper: None,
                        write_sleeper: None,
                    }
                } else {
                    warn!("Failed to register FD {fd:?} for async events");
                    return Err(e.into_parts().1);
                }
            }
        };
        Ok(MyAsyncFd {
            inner,
            need_to_restore_blocking_mode,
        })
    }
}

const FORCED_ASYNC_FD_SLEEP_POLLING: Duration = Duration::from_millis(77);

impl AsyncRead for MyAsyncFd {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        trace!("custom async fd: read");

        let finish_reading = |buf: &mut tokio::io::ReadBuf<'_>, mut n: usize| {
            if n > buf.capacity() {
                warn!("read syscall for async-fd: returned unrealistacally large number of bytes");
                n = buf.capacity();
            }
            unsafe {
                buf.assume_init(n);
            }
            buf.advance(n);
        };

        match this.inner {
            MyAsyncFdWay::Proper(ref mut f) => loop {
                let mut ready_guard = ready!(f.poll_read_ready(cx)?);

                match ready_guard.try_io(|inner| {
                    let ptr = unsafe { buf.unfilled_mut() }.as_ptr() as *mut c_void;
                    let len = buf.capacity();
                    match unsafe { read(inner.get_ref().as_raw_fd(), ptr, len) } {
                        x if x < 0 => Err(std::io::Error::last_os_error()),
                        x => Ok(x as usize),
                    }
                }) {
                    Ok(Ok(n)) => {
                        finish_reading(buf, n);

                        return Poll::Ready(Ok(()));
                    }
                    Ok(Err(e)) => return Poll::Ready(Err(e)),
                    Err(_would_block) => continue,
                }
            },
            MyAsyncFdWay::Hacky {
                ref mut fd,
                ref mut read_sleeper,
                ..
            } => loop {
                if let Some(sl) = read_sleeper.as_mut() {
                    ready!(sl.as_mut().poll(cx));
                    *read_sleeper = None;
                }

                let ptr = unsafe { buf.unfilled_mut() }.as_ptr() as *mut c_void;
                let len = buf.capacity();

                match unsafe { read(fd.as_raw_fd(), ptr, len) } {
                    x if x < 0 => {
                        let e = std::io::Error::last_os_error();
                        if e.kind() == ErrorKind::WouldBlock {
                            *read_sleeper =
                                Some(Box::pin(tokio::time::sleep(FORCED_ASYNC_FD_SLEEP_POLLING)));
                            continue;
                        }
                        return Poll::Ready(Err(e));
                    }
                    n => {
                        let n = n as usize;
                        finish_reading(buf, n);
                        return Poll::Ready(Ok(()));
                    }
                }
            },
        }
    }
}

impl AsyncWrite for MyAsyncFd {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let iov: [IoSlice<'_>; 1] = [IoSlice::new(buf)];
        self.poll_write_vectored(cx, &iov[..])
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        return Poll::Ready(Ok(()));
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        debug!("reached write shutdown of custom async fd");
        return Poll::Ready(Ok(()));
    }

    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        trace!("custom async fd: write");

        let this = self.get_mut();
        match this.inner {
            MyAsyncFdWay::Proper(ref mut f) => loop {
                let mut ready_guard = ready!(f.poll_write_ready(cx)?);

                match ready_guard
                    .try_io(|inner| writev(inner, bufs).map_err(|x| std::io::Error::from(x)))
                {
                    Ok(result) => return Poll::Ready(result),
                    Err(_would_block) => continue,
                }
            },
            MyAsyncFdWay::Hacky {
                ref mut fd,
                ref mut write_sleeper,
                ..
            } => loop {
                if let Some(sl) = write_sleeper.as_mut() {
                    ready!(sl.as_mut().poll(cx));
                    *write_sleeper = None;
                }
                match writev(&fd, bufs).map_err(|x| std::io::Error::from(x)) {
                    Ok(n) => return Poll::Ready(Ok(n)),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        *write_sleeper =
                            Some(Box::pin(tokio::time::sleep(FORCED_ASYNC_FD_SLEEP_POLLING)));
                    }
                    Err(e) => return Poll::Ready(Err(e)),
                }
            },
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }
}

//@ Use specified file descriptor for input/output, returning a StreamSocket.
//@
//@ If you want it as a DatagramSocket, just wrap it in a `chunks` wrapper.
//@
//@ May cause unsound behaviour if misused.
fn async_fd(ctx: NativeCallContext, fd: i64, force: bool) -> RhResult<Handle<StreamSocket>> {
    let ff = fd as RawFd;
    let Ok(f) = (unsafe { MyAsyncFd::new(ff, force) }) else {
        return Err(ctx.err("Failed to wrap a fd using async_fd"));
    };

    let (r, w) = tokio::io::split(f);

    let s = StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(r),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(w),
        }),
        close: None,
        fd: unsafe { SocketFd::from_i64(fd) },
    };
    debug!(s=?s, "wrapped async_fd");
    Ok(Some(s).wrap())
}

pub fn register(engine: &mut Engine) {
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("connect_seqpacket", connect_seqpacket);
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("listen_seqpacket", listen_seqpacket);

    engine.register_fn("async_fd", async_fd);
}
