use std::{
    ops::Range,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use futures::{future::OptionFuture, FutureExt};
use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::io::AsyncWriteExt;
use tracing::{debug, debug_span, error, field, warn, Instrument, Span};

use crate::scenario_executor::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, StreamRead, StreamSocket,
        StreamWrite, Task,
    },
    utils::{ExtractHandleOrFail, HandleExt2, PacketWriteExt, TaskHandleExt},
};

use super::{types::DatagramSocket, utils::RhResult};

//@ Forward unframed bytes from source to sink
fn copy_bytes(
    //@ stream source to read from
    from: Handle<StreamRead>,
    //@ stream sink to write to
    to: Handle<StreamWrite>,
    //@ task that finishes when forwarding finishes or exists with an error
) -> Handle<Task> {
    let span = debug_span!("copy_bytes", f = field::Empty, t = field::Empty);
    debug!(parent: &span, "node created");
    async move {
        let (f, t) = (from.lut(), to.lut());

        if let Some(f) = f.as_ref() {
            span.record("f", tracing::field::debug(f));
        }
        if let Some(t) = t.as_ref() {
            span.record("t", tracing::field::debug(t));
        }

        debug!(parent: &span, "node started");

        if let (Some(mut r), Some(mut w)) = (f, t) {
            if !r.prefix.is_empty() {
                match w
                    .writer
                    .write_all_buf(&mut r.prefix)
                    .instrument(span.clone())
                    .await
                {
                    Ok(()) => debug!(parent: &span, "prefix_written"),
                    Err(e) => {
                        error!(parent: &span, error=%e, "error writing prefix");
                        return;
                    }
                }
            }

            let fut = tokio::io::copy(&mut r.reader, &mut w.writer);
            let fut = fut.instrument(span.clone());

            match fut.await {
                Ok(x) => debug!(parent: &span, nbytes=x, "finished"),
                Err(e) => {
                    error!(parent: &span, error=%e, "error copying bytes");
                    return;
                }
            }
        } else {
            debug!(parent: &span, "no operation");
        }
    }
    .wrap_noerr()
}

struct ForwardingDirection<R, W> {
    r: R,
    w: W,
    bufsize: usize,
}
struct ForwardingChoiceOutcome<R, W> {
    d: Option<ForwardingDirection<R, W>>,
    unneeded_r: Option<R>,
    unneeded_w: Option<W>,
}
impl<R, W> ForwardingChoiceOutcome<R, W> {
    fn decide(r: Option<R>, w: Option<W>, enabled: bool, bufsize: usize) -> Self {
        match (enabled, r, w) {
            (true, Some(r), Some(w)) => Self {
                d: Some(ForwardingDirection { r, w, bufsize }),
                unneeded_r: None,
                unneeded_w: None,
            },
            (true, r, w) => {
                warn!("Incomplete socket specified");
                Self {
                    d: None,
                    unneeded_r: r,
                    unneeded_w: w,
                }
            }
            (false, r, w) => Self {
                d: None,
                unneeded_r: r,
                unneeded_w: w,
            },
        }
    }
}

fn exchange_bytes(
    ctx: NativeCallContext,
    opts: Dynamic,
    s1: Handle<StreamSocket>,
    s2: Handle<StreamSocket>,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("exchange_bytes", s1 = field::Empty, s2 = field::Empty);

    #[derive(serde::Deserialize)]
    struct ExchangeBytesOpts {
        //@ Transfer data only from s1 to s2
        #[serde(default)]
        pub unidirectional: bool,

        //@ Transfer data only from s2 to s1
        #[serde(default)]
        pub unidirectional_reverse: bool,

        //@ abort one transfer direction when the other reached EOF
        #[serde(default)]
        pub exit_on_eof: bool,

        //@ keep inactive transfer direction handles open
        #[serde(default)]
        pub unidirectional_late_drop: bool,

        //@ allocate this amount of buffers for transfer from s1 to s2
        pub buffer_size_forward: Option<usize>,

        //@ allocate this amount of buffers for transfer from s2 to s1
        pub buffer_size_reverse: Option<usize>,
    }
    let s1 = ctx.lutbar(s1)?;
    let s2 = ctx.lutbar(s2)?;
    let opts: ExchangeBytesOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "node created");
    Ok(async move {
        span.record("s1", tracing::field::debug(&s1));
        span.record("s2", tracing::field::debug(&s2));

        debug!(parent: &span, "node started");

        let c1 = s1.close;
        let c2 = s2.close;

        let bufsize_forward = opts.buffer_size_forward.unwrap_or(8192);
        let bufsize_reverse = opts.buffer_size_reverse.unwrap_or(8192);
        let dir1 = ForwardingChoiceOutcome::decide(
            s1.read,
            s2.write,
            !opts.unidirectional_reverse,
            bufsize_forward,
        );
        let dir2 = ForwardingChoiceOutcome::decide(
            s2.read,
            s1.write,
            !opts.unidirectional,
            bufsize_reverse,
        );

        let late_writers_shutdown = if !opts.unidirectional_late_drop {
            if let Some(x) = dir1.unneeded_r {
                drop(x)
            }
            if let Some(x) = dir2.unneeded_r {
                drop(x)
            }
            if let Some(mut x) = dir1.unneeded_w {
                let _ = x.writer.shutdown().await;
                drop(x)
            }
            if let Some(mut x) = dir2.unneeded_w {
                let _ = x.writer.shutdown().await;
                drop(x)
            }
            (None, None)
        } else {
            (dir1.unneeded_w, dir2.unneeded_w)
        };

        let mut s1;
        let mut s2;
        let mut rb1;
        let mut rb2;
        let mut w1;
        let mut w2;
        let mut copier_duplex: OptionFuture<_> = None.into();
        let mut copier_duplex_present = false;
        let mut copier1: OptionFuture<_> = None.into();
        let mut copier1_present = false;
        let mut copier2: OptionFuture<_> = None.into();
        let mut copier2_present = false;
        let hangup1_present = c1.is_some();
        let hangup1: OptionFuture<_> = c1.into();
        let hangup2_present = c2.is_some();
        let hangup2: OptionFuture<_> = c2.into();
        let mut skip_whole = false;

        match (dir1.d, dir2.d) {
            (Some(d1), Some(d2)) => {
                if !opts.exit_on_eof {
                    s1 = tokio::io::join(d1.r, d2.w.writer);
                    s2 = tokio::io::join(d2.r, d1.w.writer);
                    copier_duplex = Some(
                        tokio::io::copy_bidirectional_with_sizes(
                            &mut s1, &mut s2, d1.bufsize, d2.bufsize,
                        )
                        .instrument(span.clone()),
                    )
                    .into();
                    copier_duplex_present = true;
                } else {
                    rb1 = tokio::io::BufReader::with_capacity(d1.bufsize, d1.r);
                    rb2 = tokio::io::BufReader::with_capacity(d2.bufsize, d2.r);
                    w2 = d1.w.writer;
                    w1 = d2.w.writer;
                    copier1 = Some(tokio::io::copy_buf(&mut rb1, &mut w2)).into();
                    copier1_present = true;
                    copier2 = Some(tokio::io::copy_buf(&mut rb2, &mut w1)).into();
                    copier2_present = true;
                }
            }
            (None, Some(d)) | (Some(d), None) => {
                rb1 = tokio::io::BufReader::with_capacity(d.bufsize, d.r);
                w2 = d.w.writer;
                copier1 = Some(tokio::io::copy_buf(&mut rb1, &mut w2)).into();
                copier1_present = true;
            }
            (None, None) => skip_whole = true,
        }

        if !skip_whole {
            tokio::select! { biased;
                Some(ret) = copier_duplex, if copier_duplex_present  => {
                    match ret {
                        Ok((n1,n2)) => debug!(parent: &span, nbytes1=n1, nbytes2=n2, "finished"),
                        Err(e) =>  debug!(parent: &span, error=%e, "error"),
                    }
                }
                Some(ret) = copier1, if copier1_present  => {
                    match ret {
                        Ok(n) => debug!(parent: &span, nbytes1=n, "finished"),
                        Err(e) =>  debug!(parent: &span, error=%e, "error"),
                    }
                }
                Some(ret) = copier2, if copier2_present  => {
                    match ret {
                        Ok(n) => debug!(parent: &span, nbytes2=n, "finished"),
                        Err(e) =>  debug!(parent: &span, error=%e, "error"),
                    }
                }
                Some(()) = hangup1, if hangup1_present => {
                    debug!(parent: &span, "hangup1");
                }
                Some(()) = hangup2, if hangup2_present => {
                    debug!(parent: &span, "hangup1");
                }
            }
        }

        if let Some(mut x) = late_writers_shutdown.0 {
            let _ = x.writer.shutdown().await;
            drop(x)
        }
        if let Some(mut x) = late_writers_shutdown.1 {
            let _ = x.writer.shutdown().await;
            drop(x)
        }
    }
    .wrap_noerr())
}

#[derive(Clone)]
enum Phase {
    ReadFromStream,
    WriteToSink(Range<usize>),
}
struct CopyPackets {
    r: DatagramRead,
    w: DatagramWrite,
    first_poll: bool,
    span: tracing::Span,
    phase: Phase,
    flags: BufferFlags,
    b: Box<[u8]>,
    counter: u64,
}

impl CopyPackets {
    fn new(r: DatagramRead, w: DatagramWrite, span: Span, buffer_size: usize) -> CopyPackets {
        let b = vec![0u8; buffer_size].into_boxed_slice();

        let phase = Phase::ReadFromStream;
        let flags = crate::scenario_executor::types::BufferFlags::default();

        CopyPackets {
            r,
            w,
            first_poll: true,
            span,
            phase,
            flags,
            b,
            counter: 0,
        }
    }
}

impl std::future::Future for CopyPackets {
    type Output = u64;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u64> {
        let this = self.get_mut();

        if this.first_poll {
            this.first_poll = false;
            debug!(parent: &this.span, "node started");
        }

        loop {
            match this.phase.clone() {
                Phase::ReadFromStream => {
                    match crate::scenario_executor::types::PacketRead::poll_read(
                        this.r.src.as_mut(),
                        cx,
                        &mut this.b[..],
                    ) {
                        Poll::Ready(Ok(f)) => {
                            this.flags = f.flags;
                            this.phase = Phase::WriteToSink(f.buffer_subset);
                        }
                        Poll::Ready(Err(e)) => {
                            error!(parent: &this.span, "error reading from stream: {e}");
                            return Poll::Ready(this.counter);
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Phase::WriteToSink(range) => {
                    match crate::scenario_executor::types::PacketWrite::poll_write(
                        this.w.snk.as_mut(),
                        cx,
                        &mut this.b[range],
                        this.flags,
                    ) {
                        Poll::Ready(Ok(())) => {
                            if this.flags.contains(BufferFlag::Eof) {
                                debug!(parent: &this.span, "finished");
                                return Poll::Ready(this.counter);
                            }
                            this.phase = Phase::ReadFromStream;
                            this.counter += 1;
                        }
                        Poll::Ready(Err(e)) => {
                            error!(parent: &this.span, "error writing to sink: {e}");
                            return Poll::Ready(this.counter);
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
            };
        }
    }
}

fn copy_packets(from: Handle<DatagramRead>, to: Handle<DatagramWrite>) -> Handle<Task> {
    let span = debug_span!("copy_packets");
    debug!(parent: &span, "node created");
    let (f, t) = (from.lut(), to.lut());

    if let (Some(f), Some(t)) = (f.as_ref(), t.as_ref()) {
        debug!(parent: &span, ?f, ?t, "streams");
    }

    if let (Some(r), Some(w)) = (f, t) {
        CopyPackets::new(r, w, span, 65536)
            .map(|npkts| debug!(npkts, "finished copying packets"))
            .wrap_noerr()
    } else {
        warn!(parent: &span, "Nothing to copy");
        Arc::new(Mutex::new(None))
    }
}

fn exchange_packets(
    ctx: NativeCallContext,
    opts: Dynamic,
    s1: Handle<DatagramSocket>,
    s2: Handle<DatagramSocket>,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("exchange_packets",);

    #[derive(serde::Deserialize)]
    struct ExchangePacketsOpts {
        //@ Transfer data only from s1 to s2
        #[serde(default)]
        pub unidirectional: bool,

        //@ Transfer data only from s2 to s1
        #[serde(default)]
        pub unidirectional_reverse: bool,

        //@ abort one transfer direction when the other reached EOF
        #[serde(default)]
        pub exit_on_eof: bool,

        //@ keep inactive transfer direction handles open
        #[serde(default)]
        pub unidirectional_late_drop: bool,

        //@ allocate this amount of buffers for transfer from s1 to s2
        pub buffer_size_forward: Option<usize>,

        //@ allocate this amount of buffers for transfer from s2 to s1
        pub buffer_size_reverse: Option<usize>,
    }
    let s1 = ctx.lutbar(s1)?;
    let s2 = ctx.lutbar(s2)?;
    let opts: ExchangePacketsOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "node created");
    Ok(async move {
        span.record("s1", tracing::field::debug(&s1));
        span.record("s2", tracing::field::debug(&s2));

        debug!(parent: &span, "node started");

        let c1 = s1.close;
        let c2 = s2.close;

        let bufsize_forward = opts.buffer_size_forward.unwrap_or(32768);
        let bufsize_reverse = opts.buffer_size_reverse.unwrap_or(32768);
        let dir1 = ForwardingChoiceOutcome::decide(
            s1.read,
            s2.write,
            !opts.unidirectional_reverse,
            bufsize_forward,
        );
        let dir2 = ForwardingChoiceOutcome::decide(
            s2.read,
            s1.write,
            !opts.unidirectional,
            bufsize_reverse,
        );

        let late_writers_shutdown = if !opts.unidirectional_late_drop {
            if let Some(x) = dir1.unneeded_r {
                drop(x)
            }
            if let Some(x) = dir2.unneeded_r {
                drop(x)
            }
            if let Some(mut x) = dir1.unneeded_w {
                let _ = x.snk.as_mut().send_eof().await;
                drop(x)
            }
            if let Some(mut x) = dir2.unneeded_w {
                let _ = x.snk.as_mut().send_eof().await;
                drop(x)
            }
            (None, None)
        } else {
            (dir1.unneeded_w, dir2.unneeded_w)
        };

        let copier1_;
        let copier2_;
        let mut copier_duplex: OptionFuture<_> = None.into();
        let mut copier_duplex_present = false;
        let mut copier1: OptionFuture<_> = None.into();
        let mut copier1_present = false;
        let mut copier2: OptionFuture<_> = None.into();
        let mut copier2_present = false;
        let hangup1_present = c1.is_some();
        let hangup1: OptionFuture<_> = c1.into();
        let hangup2_present = c2.is_some();
        let hangup2: OptionFuture<_> = c2.into();
        let mut skip_whole = false;

        match (dir1.d, dir2.d) {
            (Some(d1), Some(d2)) => {
                copier1_ = CopyPackets::new(d1.r, d1.w, span.clone(), d1.bufsize);
                copier2_ = CopyPackets::new(d2.r, d2.w, span.clone(), d2.bufsize);
                if !opts.exit_on_eof {
                    let both_copiers = futures::future::join(copier1_, copier2_);

                    copier_duplex = Some(both_copiers).into();
                    copier_duplex_present = true;
                } else {
                    copier1 = Some(copier1_).into();
                    copier1_present = true;
                    copier2 = Some(copier2_).into();
                    copier2_present = true;
                }
            }
            (None, Some(d)) | (Some(d), None) => {
                copier1_ = CopyPackets::new(d.r, d.w, span.clone(), d.bufsize);
                copier1 = Some(copier1_).into();
                copier1_present = true;
            }
            (None, None) => skip_whole = true,
        }

        if !skip_whole {
            tokio::select! { biased;
                Some((n1, n2)) = copier_duplex, if copier_duplex_present  => {
                    debug!(parent: &span, npkts1=n1, npkts2=n2, "finished")
                }
                Some(n) = copier1, if copier1_present  => {
                   debug!(parent: &span, npkts1=n, "finished")
                }
                Some(n) = copier2, if copier2_present  => {
                   debug!(parent: &span, npkts2=n, "finished")
                }
                Some(()) = hangup1, if hangup1_present => {
                    debug!(parent: &span, "hangup1");
                }
                Some(()) = hangup2, if hangup2_present => {
                    debug!(parent: &span, "hangup1");
                }
            }
        }

        if let Some(mut x) = late_writers_shutdown.0 {
            let _ = x.snk.as_mut().send_eof().await;
            drop(x)
        }
        if let Some(mut x) = late_writers_shutdown.1 {
            let _ = x.snk.as_mut().send_eof().await;
            drop(x)
        }

        /*
        if let (
            Some(DatagramSocket {
                read: Some(r1),
                write: Some(w1),
                close: c1,
            }),
            Some(DatagramSocket {
                read: Some(r2),
                write: Some(w2),
                close: c2,
            }),
        ) = (s1, s2)
        {
            let copier1 = CopyPackets::new(r1, w2, span.clone(), 65536);
            let copier2 = CopyPackets::new(r2, w1, span.clone(), 65536);
            let both_copiers = futures::future::join(copier1, copier2);

            let hangup1_present = c1.is_some();
            let hangup1: OptionFuture<_> = c1.into();
            let hangup2_present = c2.is_some();
            let hangup2: OptionFuture<_> = c2.into();
            let mut skip_whole = false;

            if !skip_whole {
                tokio::select! { biased;
                    (npkts1,npkts2) = both_copiers => {
                        debug!(npkts1, npkts2, "all directions finished");
                    }
                    Some(()) = hangup1, if hangup1_present => {
                        debug!(parent: &span, "hangup1");
                    }
                    Some(()) = hangup2, if hangup2_present => {
                        debug!(parent: &span, "hangup1");
                    }
                }
            }
        } else {
            error!(parent: &span, "Incomplete datagram sockets specified");
        }
        */
    }
    .wrap_noerr())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("copy_bytes", copy_bytes);
    engine.register_fn("exchange_bytes", exchange_bytes);
    engine.register_fn("copy_packets", copy_packets);
    engine.register_fn("exchange_packets", exchange_packets);
}
