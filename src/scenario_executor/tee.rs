use std::{
    pin::Pin,
    task::{ready, Poll},
};

use futures::FutureExt;
use rhai::{Dynamic, Engine, NativeCallContext};
use tracing::{debug, debug_span, error, trace, warn};

use crate::scenario_executor::{
    types::Hangup,
    utils1::{ExtractHandleOrFail, HandleExt, SimpleErr},
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite,
    },
    utils1::RhResult,
};

struct TeeWriterNode {
    w: Option<DatagramWrite>,
    a_write_completed: bool,
}
struct TeeWriter {
    nodes: Vec<TeeWriterNode>,
    fail_all_if_one_fails: bool,
    writing_in_progress: bool,
}

impl PacketWrite for TeeWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();

        if !this.writing_in_progress {
            this.writing_in_progress = true;
            for n in &mut this.nodes {
                n.a_write_completed = false;
            }
        }

        let mut ok_count = 0;
        let mut pend_count = 0;
        let mut err_count = 0;
        let mut err = None;
        for n in &mut this.nodes {
            if n.a_write_completed {
                continue;
            }
            let Some(ref mut w) = n.w else { continue };
            match w.snk.as_mut().poll_write(cx, buf, flags) {
                Poll::Ready(Ok(())) => {
                    ok_count += 1;
                    n.a_write_completed = true;
                }
                Poll::Ready(Err(e)) => {
                    err_count += 1;
                    if this.fail_all_if_one_fails {
                        return Poll::Ready(Err(e));
                    } else {
                        n.w = None;
                        err = Some(e);
                    }
                }
                Poll::Pending => {
                    pend_count += 1;
                }
            }
        }

        trace!("ok={ok_count}, pend={pend_count}, err={err_count}");
        if pend_count > 0 {
            Poll::Pending
        } else if ok_count == 0 && err_count > 0 {
            Poll::Ready(Err(err.unwrap()))
        } else {
            this.writing_in_progress = false;
            Poll::Ready(Ok(()))
        }
    }
}

struct TeeReader {
    nodes: Vec<Option<DatagramRead>>,
    propagate_any_eof: bool,
    err_on_orphaned_fragment: bool,
    err_on_any_error: bool,
    // Node that started producing a message, not have not yet completed it (non final chunk).
    chosen_node: Option<usize>,
    active_nodes_remains: usize,
}

impl PacketRead for TeeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();

        if let Some(i) = this.chosen_node {
            trace!("reading from chosen node {i}");
            let Some(ref mut r) = this.nodes[i] else {
                unreachable!()
            };

            match ready!(r.src.as_mut().poll_read(cx, buf)) {
                Ok(prr) => {
                    if !prr.flags.contains(BufferFlag::NonFinalChunk) {
                        this.chosen_node = None;
                    }
                    if prr.flags.contains(BufferFlag::Eof)
                        && !this.err_on_orphaned_fragment
                        && !this.propagate_any_eof
                    {
                        warn!("Trimmed datagram coming from `tee:` overlay due to one of the nodes abruptly disconnecting");
                        this.chosen_node = None;
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: Default::default(),
                            buffer_subset: 0..0,
                        }));
                    }
                    return Poll::Ready(Ok(prr));
                }
                Err(e) => {
                    if this.err_on_orphaned_fragment {
                        return Poll::Ready(Err(e));
                    } else {
                        warn!("Trimmed datagram coming from `tee:` overlay due to one of the nodes abruptly disconnecting: {e}");
                        this.chosen_node = None;
                        return Poll::Ready(Ok(PacketReadResult {
                            flags: Default::default(),
                            buffer_subset: 0..0,
                        }));
                    }
                }
            }
        }

        // no chosen node: all can potentially produce a packet

        let mut ok_count = 0;
        let mut frag_count = 0;
        let mut eof_count = 0;
        let mut pend_count = 0;
        let mut err_count = 0;
        let mut err_to_propagate = None;
        let mut prr_to_propagate = None;

        for (i, n) in this.nodes.iter_mut().enumerate() {
            let annul;
            let mut eof_occured = false;
            {
                let Some(r) = n.as_mut() else {
                    continue;
                };

                match r.src.as_mut().poll_read(cx, buf) {
                    Poll::Pending => {
                        pend_count += 1;
                        // FIXME: poll_read may mutate pending buffer that may be used by another pending neighbouring node
                        continue;
                    }
                    Poll::Ready(Ok(prr)) => {
                        prr_to_propagate = Some(prr.clone());
                        if prr.flags.contains(BufferFlag::NonFinalChunk) {
                            frag_count += 1;
                            this.chosen_node = Some(i);
                            break;
                        } else if prr.flags.contains(BufferFlag::Eof) {
                            eof_count += 1;
                            eof_occured = true;
                            annul = true;
                        } else {
                            ok_count += 1;
                            break;
                        }
                    }
                    Poll::Ready(Err(e)) => {
                        annul = true;
                        err_to_propagate = Some(e);
                        err_count += 1;
                    }
                }
            }
            if annul {
                *n = None;
                this.active_nodes_remains -= 1;
            }
            if eof_occured && this.propagate_any_eof {
                break;
            }
        }
        trace!("ok={ok_count} frag={frag_count} eof={eof_count} pend={pend_count} err={err_count}");

        #[allow(clippy::if_same_then_else)]
        if this.err_on_any_error && err_count > 0 {
            Poll::Ready(Err(err_to_propagate.unwrap()))
        } else if this.propagate_any_eof && eof_count > 0 {
            Poll::Ready(Ok(prr_to_propagate.unwrap()))
        } else if ok_count > 0 || frag_count > 0 {
            Poll::Ready(Ok(prr_to_propagate.unwrap()))
        } else if eof_count > 0 && this.active_nodes_remains == 0 {
            Poll::Ready(Ok(prr_to_propagate.unwrap()))
        } else if err_count > 0 && this.active_nodes_remains == 0 {
            Poll::Ready(Err(err_to_propagate.unwrap()))
        } else if pend_count > 0 {
            Poll::Pending
        } else {
            error!("strange tee's read state: ok={ok_count} frag={frag_count} eof={eof_count} pend={pend_count} err={err_count}");
            Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
        }
    }
}

//@ Combine multiple datagram sockets into one that writes to all specified inner sockets and reads from any of them.
fn tee(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Array of `DatagramSocket`s
    sockets: Vec<Dynamic>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("tee");

    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Disconnect all the inner socket writers if one hangs up or fails a write
        #[serde(default)]
        write_fail_all_if_one_fails: bool,

        //@ Disconnect all the inner socket writers if one hangs up or fails a write
        #[serde(default)]
        read_fail_all_if_one_fails: bool,

        //@ If one of `tee:`'s branches signed EOF, propagate it to user
        #[serde(default)]
        propagate_eofs: bool,

        //@ If a node starts emitting a datagram, then goes away; do not abort the reading side
        //@ of the connection, instead just make a trimmed, corrupted message and continue.
        #[serde(default)]
        tolerate_torn_msgs: bool,

        //@ Cause hangup if any of the branches signals a hangup
        #[serde(default)]
        use_hangups: bool,

        //@ Use hangup token specifically from the first of specified sockets
        #[serde(default)]
        use_first_hangup: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    let mut ss: Vec<DatagramSocket> = Vec::with_capacity(sockets.len());

    for (i, socket) in sockets.into_iter().enumerate() {
        let Some(socket): Option<Handle<DatagramSocket>> = socket.try_cast() else {
            return Err(ctx.err("Non-datagram-socket element in the `tee`'s array"));
        };

        debug!(i, "extracted socket");
        let s = ctx.lutbar(socket)?;
        ss.push(s);
    }

    if opts.use_hangups && opts.use_first_hangup {
        return Err(ctx.err("use_hangups and use_first_hangup are incompatible"));
    }

    if ss.is_empty() {
        return Err(ctx.err("Specify at least one socket"));
    }

    debug!(parent: &span, "options parsed");

    let mut close: Option<Hangup> = None;

    if opts.use_hangups {
        let mut hgtoks: Vec<Hangup> = vec![];
        for s in &mut ss {
            if let Some(hg) = s.close.take() {
                hgtoks.push(hg);
            }
        }

        close = if hgtoks.is_empty() {
            None
        } else if hgtoks.len() == 1 {
            Some(hgtoks.drain(..).next().unwrap())
        } else {
            Some(Box::pin(futures::future::select_all(hgtoks).map(|_| ())))
        };
    }
    if opts.use_first_hangup {
        close = ss[0].close.take();
    }

    let mut write_nodes = Vec::with_capacity(ss.len());
    let mut read_nodes = Vec::with_capacity(ss.len());

    for s in ss {
        if let Some(r) = s.read {
            read_nodes.push(Some(r));
        }
        if let Some(w) = s.write {
            write_nodes.push(TeeWriterNode {
                w: Some(w),
                a_write_completed: false,
            });
        }
    }

    let write = DatagramWrite {
        snk: Box::pin(TeeWriter {
            nodes: write_nodes,
            fail_all_if_one_fails: opts.write_fail_all_if_one_fails,
            writing_in_progress: false,
        }),
    };
    let active_nodes_remains = read_nodes.len();
    let read = DatagramRead {
        src: Box::pin(TeeReader {
            nodes: read_nodes,
            propagate_any_eof: opts.propagate_eofs,
            err_on_orphaned_fragment: !opts.tolerate_torn_msgs,
            err_on_any_error: opts.read_fail_all_if_one_fails,
            chosen_node: None,
            active_nodes_remains,
        }),
    };

    let s = DatagramSocket {
        read: Some(read),
        write: Some(write),
        close,
        fd: None,
    };

    debug!(parent: &span, ?s, "wrapped");

    Ok(Some(s).wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("tee", tee);
}
