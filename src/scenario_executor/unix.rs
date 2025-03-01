use std::{
    ffi::{OsStr, OsString},
    sync::Arc,
    task::{ready, Poll},
    time::Duration,
};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils1::{wrap_as_stream_socket, SimpleErr, TaskHandleExt2},
    utils2::AddressOrFd,
};
use futures::FutureExt;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::net::UnixStream;
use tracing::{debug, debug_span, error, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

use super::{
    types::{BufferFlag, BufferFlags, PacketRead, PacketReadResult, PacketWrite},
    utils1::{RhResult, SignalOnDrop},
    utils2::{Defragmenter, DefragmenterAddChunkResult},
};
use clap_lex::OsStrExt;

fn abstractify(path: &mut OsString) {
    let tmp = std::mem::take(path);
    *path = std::os::unix::ffi::OsStringExt::from_vec(vec![0]);
    path.push(tmp);
}

async fn maybe_chmod<T>(
    chmod: Option<u32>,
    path: &OsStr,
    mut acceptor: impl FnMut() -> Option<std::io::Result<T>>,
) -> Result<(), anyhow::Error> {
    if let Some(chmod) = chmod {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::set_permissions(path, std::fs::Permissions::from_mode(chmod)) {
            Ok(_) => {
                debug!(?path, chmod, "chmod");
            }
            Err(e) => {
                error!("Failed to chmod {path:?} to {chmod}: {e}");
                return Err(e.into());
            }
        }

        if chmod != 0o666 {
            // Throw away potential sneaky TOCTOU connections that got through before we issued chmod.
            // I'm not sure about if this scheme is waterproof.

            loop {
                let Some(c) = acceptor() else { break };
                if let Err(e) = c {
                    error!(%e, "Error from accept");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                } else {
                    warn!("Rejected incoming connection to UNIX socket that may have happened before we did chmod");
                }
            }
        }
    }
    Ok(())
}

//@ Connect to a UNIX stream socket of some kind
fn connect_unix(
    ctx: NativeCallContext,
    opts: Dynamic,
    mut path: OsString,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_unix");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct UnixOpts {
        //@ On Linux, connect to an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,
    }
    let opts: UnixOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, ?path, r#abstract=opts.r#abstract, "options parsed");

    if opts.r#abstract {
        abstractify(&mut path);
    }

    Ok(async move {
        debug!("node started");
        let t = UnixStream::connect(path).await?;
        let (r, w) = t.into_split();
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Listen UNIX or abstract socket
fn listen_unix(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Path to a socket file to create, name of abstract address to use or empty string if `fd` is used.
    mut path: OsString,
    //@ Called once after the port is bound
    when_listening: FnPtr,
    //@ Called on each accepted connection
    on_accept: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("listen_unix");
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

        //@ On Linux, listen an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,

        //@ Change filesystem mode (permissions) of the file after listening
        #[serde(default)]
        chmod: Option<u32>,

        //@ Automatically spawn a task for each accepted connection
        #[serde(default)]
        autospawn: bool,

        //@ Exit listening loop after processing a single connection
        #[serde(default)]
        oneshot: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));

    let autospawn = opts.autospawn;

    if opts.r#abstract {
        abstractify(&mut path);
    }

    let a =
        AddressOrFd::interpret_path(&ctx, &span, path, opts.fd, opts.named_fd, opts.r#abstract)?;

    Ok(async move {
        debug!("node started");

        let assertaddr = Some(ListenFromFdType::Unix);
        let forceaddr = if opts.fd_force { assertaddr } else { None };
        let l = match &a {
            AddressOrFd::Addr(path) => tokio::net::UnixListener::bind(path)?,
            AddressOrFd::Fd(f) => {
                unsafe { listen_from_fd(*f, forceaddr, assertaddr) }?.unwrap_unix()
            }
            AddressOrFd::NamedFd(f) => {
                unsafe { listen_from_fd_named(f, forceaddr, assertaddr) }?.unwrap_unix()
            }
        };

        if let Some(path) = a.addr() {
            maybe_chmod(opts.chmod, path, || l.accept().now_or_never()).await?;
        }

        callback_and_continue::<()>(the_scenario.clone(), when_listening, ()).await;

        let mut drop_nofity = None;

        loop {
            let the_scenario = the_scenario.clone();
            let on_accept = on_accept.clone();
            match l.accept().await {
                Ok((t, from)) => {
                    let newspan = debug_span!("unix_accept", from=?from);
                    let (r, w) = t.into_split();
                    let (r, w) = (Box::pin(r), Box::pin(w));

                    let (s, dn) = wrap_as_stream_socket(r, w, None, opts.oneshot);
                    drop_nofity = dn;

                    debug!(parent: &newspan, s=?s,"accepted");
                    let h = s.wrap();
                    if !autospawn {
                        callback_and_continue::<(Handle<StreamSocket>,)>(
                            the_scenario,
                            on_accept,
                            (h,),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<StreamSocket>,)>(
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

            if opts.oneshot {
                debug!("Exiting UNIX listener due to --oneshot mode");
                break;
            }
        }
        if let Some((dn1, dn2)) = drop_nofity {
            debug!("Waiting for the sole accepted client to finish serving reads");
            let _ = dn1.await;
            debug!("Waiting for the sole accepted client to finish serving writes");
            let _ = dn2.await;
            debug!("The sole accepted client finished");
        }
        Ok(())
    }
    .instrument(span)
    .wrap())
}

fn unlink_file(
    ctx: NativeCallContext,
    path: OsString,
    //@ Emit error if unlinking fails.
    bail_if_fails: bool,
) -> RhResult<()> {
    match std::fs::remove_file(&path) {
        Ok(_) => {
            debug!(?path, "Unlinked file");
            Ok(())
        }
        Err(e) => {
            if bail_if_fails {
                warn!(?path, %e, "Failed to unlink");
                Err(ctx.err("failed to unlink"))
            } else {
                debug!(?path, %e, "Failed to unlink");
                Ok(())
            }
        }
    }
}

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
                return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
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
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_seqpacket");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct ConnectSeqpacketOpts {
        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
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
    } else {
        if path.starts_with("@") {
            warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
        }
    }

    Ok(async move {
        debug!("node started");
        let s = tokio_seqpacket::UnixSeqpacket::connect(path).await?;
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

        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
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
    } else {
        if path.starts_with("@") {
            warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
        }
    }
    let a =
        AddressOrFd::interpret_path(&ctx, &span, path, opts.fd, opts.named_fd, opts.r#abstract)?;

    Ok(async move {
        debug!("node started");

        let assertaddr = Some(ListenFromFdType::Seqpacket);
        let forceaddr = if opts.fd_force { assertaddr } else { None };
        let mut l = match &a {
            AddressOrFd::Addr(path) => tokio_seqpacket::UnixSeqpacketListener::bind(&path)?,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ListenFromFdType {
    Unix,
    Seqpacket,
    Tcp,
    Udp,
}
#[derive(Debug)]
pub enum ListenFromFdOutcome {
    Unix(tokio::net::UnixListener),
    #[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
    Seqpacket(tokio_seqpacket::UnixSeqpacketListener),
    Tcp(tokio::net::TcpListener),
    Udp(tokio::net::UdpSocket),
}
impl ListenFromFdOutcome {
    pub fn unwrap_tcp(self) -> tokio::net::TcpListener {
        if let ListenFromFdOutcome::Tcp(x) = self {
            x
        } else {
            panic!()
        }
    }

    pub fn unwrap_udp(self) -> tokio::net::UdpSocket {
        if let ListenFromFdOutcome::Udp(x) = self {
            x
        } else {
            panic!()
        }
    }
    #[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
    pub fn unwrap_seqpacket(self) -> tokio_seqpacket::UnixSeqpacketListener {
        if let ListenFromFdOutcome::Seqpacket(x) = self {
            x
        } else {
            panic!()
        }
    }

    pub fn unwrap_unix(self) -> tokio::net::UnixListener {
        if let ListenFromFdOutcome::Unix(x) = self {
            x
        } else {
            panic!()
        }
    }
}

/// SATEFY: Tokio's interfal file descriptors and other io-unsafe things should not be specified as `fdnum`. Maybe `force_type` can also cause nastiness (not sure).
pub unsafe fn listen_from_fd(
    fdnum: i32,
    force_type: Option<ListenFromFdType>,
    assert_type: Option<ListenFromFdType>,
) -> Result<ListenFromFdOutcome, std::io::Error> {
    use std::os::fd::{FromRawFd, IntoRawFd, RawFd};

    use socket2::{Domain, Type};

    let fd: RawFd = (fdnum).into();

    let s = unsafe { socket2::Socket::from_raw_fd(fd) };

    let typ = match force_type {
        Some(x) => x,
        None => {
            let sa = s.local_addr().map_err(|e| {
                error!("Failed to determine socket domain of file descriptor {fd}: {e}");
                e
            })?;
            let t = s.r#type().map_err(|e| {
                error!("Failed to determine socket type of file descriptor {fd}: {e}");
                e
            })?;
            match (sa.domain(), t) {
                (Domain::UNIX, Type::STREAM) => ListenFromFdType::Unix,
                (Domain::UNIX, Type::DGRAM) => {
                    error!("File descriptor {fdnum} is an AF_UNIX datagram socket, this is currently not supported");
                    return Err(std::io::ErrorKind::Other.into());
                }
                (Domain::UNIX, Type::SEQPACKET) => ListenFromFdType::Seqpacket,
                #[cfg(any(target_os = "android", target_os = "linux"))]
                (Domain::VSOCK, _) => {
                    error!("File descriptor {fdnum} is a VSOCK socket, this is currently not supported");
                    return Err(std::io::ErrorKind::Other.into());
                }
                (Domain::IPV4 | Domain::IPV6, Type::STREAM) => ListenFromFdType::Tcp,
                (Domain::IPV4 | Domain::IPV6, Type::DGRAM) => ListenFromFdType::Udp,
                (d, t) => {
                    error!("File descriptor {fdnum} has unknown socket domain:type combination: {d:?}:{t:?}");
                    return Err(std::io::ErrorKind::Other.into());
                }
            }
        }
    };

    if let Some(at) = assert_type {
        if at != typ {
            error!("File descriptor {fd} has invalid socket type: {typ:?} instead of {at:?}");
            return Err(std::io::ErrorKind::Other.into());
        }
    }

    s.set_nonblocking(true)?;

    let fd: RawFd = s.into_raw_fd();

    Ok(match typ {
        ListenFromFdType::Unix => {
            let s = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd) };
            ListenFromFdOutcome::Unix(tokio::net::UnixListener::from_std(s)?)
        }
        ListenFromFdType::Seqpacket => {
            #[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
            {
                let s = tokio_seqpacket::UnixSeqpacketListener::from_raw_fd(fd)?;
                return Ok(ListenFromFdOutcome::Seqpacket(s));
            }
            error!("Attempt to get a SOCK_SEQPACKET on platform where it is not supported");
            return Err(std::io::ErrorKind::Other.into())
        }
        ListenFromFdType::Tcp => {
            let s = unsafe { std::net::TcpListener::from_raw_fd(fd) };
            ListenFromFdOutcome::Tcp(tokio::net::TcpListener::from_std(s)?)
        }
        ListenFromFdType::Udp => {
            let s = unsafe { std::net::UdpSocket::from_raw_fd(fd) };
            ListenFromFdOutcome::Udp(tokio::net::UdpSocket::from_std(s)?)
        }
    })
}

/// SATEFY: Tokio's interfal file descriptors and other io-unsafe things should not be specified as `fdnum`. Maybe `force_type` can also cause nastiness (not sure).
pub unsafe fn listen_from_fd_named(
    fdname: &str,
    force_type: Option<ListenFromFdType>,
    assert_type: Option<ListenFromFdType>,
) -> Result<ListenFromFdOutcome, std::io::Error> {
    const SD_LISTEN_FDS_START: i32 = 3;

    let (Ok(listen_fds), Ok(listen_fdnames)) =
        (std::env::var("LISTEN_FDS"), std::env::var("LISTEN_FDNAMES"))
    else {
        error!("Cannot get LISTEN_FDS or LISTEN_FDNAMES environment variables to determine FD of `{fdname}`");
        return Err(std::io::ErrorKind::Other.into());
    };

    let Ok(n): Result<usize, _> = listen_fds.parse() else {
        error!("Invalid value of LISTEN_FDS environment variable");
        return Err(std::io::ErrorKind::Other.into());
    };

    let mut fd: i32 = SD_LISTEN_FDS_START;
    for (i, name) in listen_fdnames.split(':').enumerate() {
        if i >= n {
            break;
        }
        debug!("Considering LISTEN_FDNAMES chunk `{name}`");
        if name == fdname {
            return listen_from_fd(fd, force_type, assert_type);
        }
        fd += 1;
    }

    error!("Named file descriptor `{fdname}` not found in LISTEN_FDNAMES");
    return Err(std::io::ErrorKind::Other.into());
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_unix", connect_unix);
    engine.register_fn("listen_unix", listen_unix);
    engine.register_fn("unlink_file", unlink_file);
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("connect_seqpacket", connect_seqpacket);
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("listen_seqpacket", listen_seqpacket);
}
