use std::{
    ffi::OsString,
    sync::Arc,
    task::{ready, Poll},
    time::Duration,
};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils1::{wrap_as_stream_socket, SimpleErr, TaskHandleExt2},
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
    path: OsString,
    mut acceptor: impl FnMut() -> Option<std::io::Result<T>>,
) -> Result<(), anyhow::Error> {
    if let Some(chmod) = chmod {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::set_permissions(&path, std::fs::Permissions::from_mode(chmod)) {
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
    struct UnixListenOpts {
        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
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
    let opts: UnixListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=?path, r#abstract=opts.r#abstract, "options parsed");

    let autospawn = opts.autospawn;

    if opts.r#abstract {
        abstractify(&mut path);
    }

    Ok(async move {
        debug!("node started");
        let l = tokio::net::UnixListener::bind(&path)?;

        maybe_chmod(opts.chmod, path, || l.accept().now_or_never()).await?;

        callback_and_continue::<()>(
            the_scenario.clone(),
            when_listening,
            (),
        )
        .await;

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
    struct SeqpacketListenOpts {
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
    let opts: SeqpacketListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=?path, r#abstract=opts.r#abstract, "options parsed");

    let autospawn = opts.autospawn;
    let oneshot = opts.oneshot;

    if opts.r#abstract {
        abstractify(&mut path);
    } else {
        if path.starts_with("@") {
            warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
        }
    }

    Ok(async move {
        debug!("node started");
        let mut l = tokio_seqpacket::UnixSeqpacketListener::bind(&path)?;

        maybe_chmod(opts.chmod, path, || l.accept().now_or_never()).await?;

        callback_and_continue::<()>(
            the_scenario.clone(),
            when_listening,
            (),
        )
        .await;

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



pub enum ListenFromFdType {
    Unix,
    Seqpacket,
    Tcp,
    Udp,
}
#[derive(Debug)]
pub enum ListenFromFdOutcome {
    Unix(tokio::net::UnixListener),
    Seqpacket(tokio_seqpacket::UnixSeqpacketListener),
    Tcp(tokio::net::TcpListener),
    Udp(tokio::net::UdpSocket),
}

/// SATEFY: Tokio's interfal file descriptors and other io-unsafe things should not be specified as `fdnum`. Maybe `force_type` can also cause nastiness (not sure).
pub unsafe fn listen_from_fd(
    fdnum: i32,
    force_type: Option<ListenFromFdType>,
) -> Result<ListenFromFdOutcome, std::io::Error> {  
    use std::os::fd::{FromRawFd, RawFd, IntoRawFd};

    use socket2::{Domain,Type};

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
                (Domain::UNIX, Type::STREAM) => {
                    ListenFromFdType::Unix
                }
                (Domain::UNIX, Type::DGRAM) => {
                    error!("File descriptor {fdnum} is an AF_UNIX datagram socket, this is currently not supported");
                    return Err(std::io::ErrorKind::Other.into())
                }
                (Domain::UNIX, Type::SEQPACKET) => {
                    ListenFromFdType::Seqpacket
                }
                (Domain::VSOCK, _) => {
                    error!("File descriptor {fdnum} is a VSOCK socket, this is currently not supported");
                    return Err(std::io::ErrorKind::Other.into())
                }
                (Domain::IPV4 | Domain::IPV6, Type::STREAM) => {
                    ListenFromFdType::Tcp
                }
                (Domain::IPV4 | Domain::IPV6, Type::DGRAM) => {
                    ListenFromFdType::Udp        
                }
                (d, t) => {
                    error!("File descriptor {fdnum} has unknown socket domain:type combination: {d:?}:{t:?}");
                    return Err(std::io::ErrorKind::Other.into())
                }
            }
        }
    };

    s.set_nonblocking(true)?;

    let fd : RawFd = s.into_raw_fd();

    Ok(match typ {
        ListenFromFdType::Unix => {
            let s = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd) };
            ListenFromFdOutcome::Unix(tokio::net::UnixListener::from_std(s)?)
        }
        ListenFromFdType::Seqpacket => {
            let s = tokio_seqpacket::UnixSeqpacketListener::from_raw_fd(fd)?;
            ListenFromFdOutcome::Seqpacket(s)
        }
        ListenFromFdType::Tcp => {
            let s = unsafe { std::net::TcpListener::from_raw_fd(fd) };
            ListenFromFdOutcome::Tcp(tokio::net::TcpListener::from_std(s)?)
        }
        ListenFromFdType::Udp => {
            let s = unsafe { std::net::UdpSocket::from_raw_fd(fd)};
            ListenFromFdOutcome::Udp(tokio::net::UdpSocket::from_std(s)?)
        }
    })
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
