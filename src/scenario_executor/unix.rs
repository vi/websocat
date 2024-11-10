use std::{ffi::OsString, sync::Arc, task::Poll, time::Duration};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils::{IsControlFrame, SimpleErr, TaskHandleExt2},
};
use bytes::BytesMut;
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
    utils::RhResult,
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

fn listen_unix(
    ctx: NativeCallContext,
    opts: Dynamic,
    mut path: OsString,
    continuation: FnPtr,
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

        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.accept().await {
                Ok((t, from)) => {
                    let newspan = debug_span!("unix_accept", from=?from);
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

                    debug!(parent: &newspan, s=?s,"accepted");
                    let h = s.wrap();
                    if !autospawn {
                        callback_and_continue::<(Handle<StreamSocket>,)>(
                            the_scenario,
                            continuation,
                            (h,),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<StreamSocket>,)>(
                                the_scenario,
                                continuation,
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
        }
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

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
#[derive(Clone)]
struct SeqpacketSendAdapter {
    s: Arc<tokio_seqpacket::UnixSeqpacket>,
    text: bool,
}

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
#[derive(Clone)]
struct SeqpacketRecvAdapter {
    s: Arc<tokio_seqpacket::UnixSeqpacket>,
    incomplete_outgoing_datagram_buffer: Option<BytesMut>,
    incomplete_outgoing_datagram_buffer_complete: bool,
}

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
impl PacketRead for SeqpacketSendAdapter {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let size = std::task::ready!(self.s.poll_recv(cx, buf))?;

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

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
impl PacketWrite for SeqpacketRecvAdapter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if flags.contains(BufferFlag::Eof) {
            this.s.shutdown(std::net::Shutdown::Write)?;
            return Poll::Ready(Ok(()));
        }
        if flags.is_control() {
            return Poll::Ready(Ok(()));
        }
        if flags.contains(BufferFlag::NonFinalChunk) {
            this.incomplete_outgoing_datagram_buffer
                .get_or_insert_with(Default::default)
                .extend_from_slice(buf);
            return Poll::Ready(Ok(()));
        }
        let data: &[u8] = if let Some(ref mut x) = this.incomplete_outgoing_datagram_buffer {
            if !this.incomplete_outgoing_datagram_buffer_complete {
                x.extend_from_slice(buf);
                this.incomplete_outgoing_datagram_buffer_complete = true;
            }
            &x[..]
        } else {
            buf
        };

        let ret = this.s.poll_send(cx, data);

        match ret {
            Poll::Ready(Ok(n)) => {
                if n != data.len() {
                    warn!("short SEQPACKET send");
                }
            }
            Poll::Ready(Err(e)) => {
                return Poll::Ready(Err(e));
            }
            Poll::Pending => return Poll::Pending,
        }

        this.incomplete_outgoing_datagram_buffer_complete = false;
        this.incomplete_outgoing_datagram_buffer = None;
        Poll::Ready(Ok(()))
    }
}

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
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
    struct UnixOpts {
        //@ On Linux, connect ot an abstract-namespaced socket instead of file-based
        #[serde(default)]
        r#abstract: bool,

        //@ Mark received datagrams as text
        #[serde(default)]
        text: bool,
    }
    let opts: UnixOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, ?path, r#abstract=opts.r#abstract, "options parsed");

    if opts.r#abstract {
        abstractify(&mut path);
        warn!("Due to https://github.com/de-vri-es/tokio-seqpacket-rs/issues/24 it may append a zero byte to abstract socket addresses.")
    } else {
        if path.starts_with("@") {
            warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
        }
    }

    Ok(async move {
        debug!("node started");
        let s = tokio_seqpacket::UnixSeqpacket::connect(path).await?;
        let s = Arc::new(s);
        let r = SeqpacketSendAdapter {
            s: s.clone(),
            text: opts.text,
        };
        let w = SeqpacketRecvAdapter {
            s,
            incomplete_outgoing_datagram_buffer: None,
            incomplete_outgoing_datagram_buffer_complete: true,
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

#[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
fn listen_seqpacket(
    ctx: NativeCallContext,
    opts: Dynamic,
    mut path: OsString,
    continuation: FnPtr,
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
    }
    let opts: SeqpacketListenOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, listen_addr=?path, r#abstract=opts.r#abstract, "options parsed");

    let autospawn = opts.autospawn;

    if opts.r#abstract {
        abstractify(&mut path);

        warn!("Due to https://github.com/de-vri-es/tokio-seqpacket-rs/issues/24 it may append a zero byte to abstract socket addresses.")
    } else {
        if path.starts_with("@") {
            warn!("Websocat4 no longer converts @-prefixed addresses to abstract namespace anymore")
        }
    }

    Ok(async move {
        debug!("node started");
        let mut l = tokio_seqpacket::UnixSeqpacketListener::bind(&path)?;

        maybe_chmod(opts.chmod, path, || l.accept().now_or_never()).await?;

        let mut i = 0;
        loop {
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            match l.accept().await {
                Ok(s) => {
                    let newspan = debug_span!("seqpacket_accept", i);
                    i += 1;
                    let s = Arc::new(s);
                    let r = SeqpacketSendAdapter {
                        s: s.clone(),
                        text: opts.text,
                    };
                    let w = SeqpacketRecvAdapter {
                        s,
                        incomplete_outgoing_datagram_buffer: None,
                        incomplete_outgoing_datagram_buffer_complete: true,
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
                            continuation,
                            (h,),
                        )
                        .instrument(newspan)
                        .await;
                    } else {
                        tokio::spawn(async move {
                            callback_and_continue::<(Handle<DatagramSocket>,)>(
                                the_scenario,
                                continuation,
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
        }
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_unix", connect_unix);
    engine.register_fn("listen_unix", listen_unix);
    engine.register_fn("unlink_file", unlink_file);
    #[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
    engine.register_fn("connect_seqpacket", connect_seqpacket);
    #[cfg(any(target_os = "linux",target_os = "android",target_os = "freebsd"))]
    engine.register_fn("listen_seqpacket", listen_seqpacket);
}
