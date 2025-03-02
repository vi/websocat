use std::{
    ffi::OsString,
    sync::Arc,
    task::{ready, Poll},
    time::Duration,
};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils1::TaskHandleExt2,
    utils2::AddressOrFd,
};
use futures::FutureExt;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, error, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, Task},
};

use super::{
    types::{BufferFlag, BufferFlags, PacketRead, PacketReadResult, PacketWrite},
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
    use crate::scenario_executor::unix1::abstractify;

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

pub fn register(engine: &mut Engine) {
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("connect_seqpacket", connect_seqpacket);
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    engine.register_fn("listen_seqpacket", listen_seqpacket);
}
