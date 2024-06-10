use std::{task::{Context, Poll}, sync::{Mutex, Arc}};

use futures::future::OptionFuture;
use rhai::Engine;
use tokio::io::{AsyncWriteExt, ReadBuf};
use tracing::{debug_span, debug, field, Instrument, error, info, warn};

use crate::types::{BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, HandleExt2, StreamRead, StreamSocket, StreamWrite, Task, TaskHandleExt};

fn copy_bytes(from: Handle<StreamRead>, to: Handle<StreamWrite>) -> Handle<Task> {
    let span = debug_span!("copy_bytes", f=field::Empty, t=field::Empty);
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
            if ! r.prefix.is_empty() {
                match w.writer.write_all_buf(&mut r.prefix).await {
                    Ok(()) => debug!(parent: &span, "prefix_written"),
                    Err(e) =>  debug!(parent: &span, error=%e, "error"),
                }
            }

            let fut = tokio::io::copy(&mut r.reader, &mut w.writer);
            let fut = fut.instrument(span.clone());

            match fut.await {
                Ok(x) => debug!(parent: &span, nbytes=x, "finished"),
                Err(e) =>  debug!(parent: &span, error=%e, "error"),
            }
        } else {
            debug!(parent: &span, "no operation");
        }
    }.wrap()
}
fn copy_bytes_bidirectional(s1: Handle<StreamSocket>, s2: Handle<StreamSocket>) -> Handle<Task> {
    let span = debug_span!("copy_bytes_bidirectional", s1=field::Empty, s2=field::Empty);
    debug!(parent: &span, "node created");
    async move {
        let (s1, s2) = (s1.lut(), s2.lut());

        if let Some(s1) = s1.as_ref() {
            span.record("s1", tracing::field::debug(s1));
        }
        if let Some(s2) = s2.as_ref() {
            span.record("s2", tracing::field::debug(s2));
        }

        debug!(parent: &span, "node started");

        if let (Some(StreamSocket { read: Some(mut r1), write: Some(mut w1), close: c1 }),
                Some(StreamSocket { read: Some(mut r2), write: Some(mut w2), close: c2 }),
            ) = (s1,s2) {

            if ! r1.prefix.is_empty() {
                match w2.writer.write_all_buf(&mut r1.prefix).await {
                    Ok(()) => debug!(parent: &span, "prefix_written_1to2"),
                    Err(e) =>  debug!(parent: &span, error=%e, "error_1to2"),
                }
            }

            if ! r2.prefix.is_empty() {
                match w1.writer.write_all_buf(&mut r2.prefix).await {
                    Ok(()) => debug!(parent: &span, "prefix_written_2to1"),
                    Err(e) =>  debug!(parent: &span, error=%e, "error_2to1"),
                }
            }

            let mut s1 = tokio::io::join(r1.reader, w1.writer);
            let mut s2 = tokio::io::join(r2.reader, w2.writer);

            let copier = tokio::io::copy_bidirectional(&mut s1, &mut s2);

            let c1p = c1.is_some();
            let c1o : OptionFuture<_> = c1.into();

            let c2p = c2.is_some();
            let c2o : OptionFuture<_> = c2.into();

            tokio::select! { biased;
                Some(()) = c1o, if c1p => {
                    debug!(parent: &span, "hangup1");
                }
                Some(()) = c2o, if c2p => {
                    debug!(parent: &span, "hangup2");
                }
                ret = copier  => {
                    match ret {
                        Ok((n1,n2)) => debug!(parent: &span, nbytes1=n1, nbytes2=n2, "finished"),
                        Err(e) =>  debug!(parent: &span, error=%e, "error"),
                    }
                }
            }

        } else {
            error!(parent: &span, "Incomplete stream sockets specified");
        }
    }.wrap()
}

enum Phase {
    ReadFromStream,
    WriteToSink(usize),
}
struct CopyPackets {
    r: DatagramRead,
    w: DatagramWrite,
    first_poll: bool,
    span: tracing::Span,
    phase: Phase,
    flags: BufferFlags,
    b: Box<[u8]>,
}

impl std::future::Future for CopyPackets {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();

        if this.first_poll {
            this.first_poll = false;
            debug!(parent: &this.span, "node started");
        }

        loop {
            match this.phase {
                Phase::ReadFromStream => {
                    let mut bb = ReadBuf::new(&mut this.b[..]);
                    match crate::types::PacketRead::poll_read(this.r.src.as_mut(), cx, &mut bb) {
                        Poll::Ready(Ok(f)) => {
                            let n = bb.filled().len();
                            drop(bb);
                            this.flags = f;
                            this.phase = Phase::WriteToSink(n);
                        }
                        Poll::Ready(Err(e)) => {
                            error!(parent: &this.span, "error reading from stream: {e}");
                            return Poll::Ready(())
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Phase::WriteToSink(n) => {
                    let mut bb = ReadBuf::new(&mut this.b[..]);
                    bb.advance(n);
                    match crate::types::PacketWrite::poll_write(this.w.snk.as_mut(), cx, &mut bb, this.flags) {
                        Poll::Ready(Ok(())) => {
                            if this.flags.contains(BufferFlag::Eof) {
                                info!(parent: &this.span, "finished");
                                return Poll::Ready(())
                            }
                            this.phase = Phase::ReadFromStream;
                        }
                        Poll::Ready(Err(e)) => {
                            error!(parent: &this.span, "error writing to sink: {e}");
                            return Poll::Ready(())
                        }
                        Poll::Pending => todo!(),
                    }
                }
            };
        }
    }
}

fn copy_packets(from: Handle<DatagramRead>, to: Handle<DatagramWrite>) -> Handle<Task> {
    let span = debug_span!("copy_packets", f=field::Empty, t=field::Empty);
    debug!(parent: &span, "node created");
    let (f, t) = (from.lut(), to.lut());

    let b = vec![0u8; 65536].into_boxed_slice();
   
    let phase = Phase::ReadFromStream;
    let flags = crate::types::BufferFlags::default();

    if let Some(f) = f.as_ref() {
        span.record("f", tracing::field::debug(f));
    }
    if let Some(t) = t.as_ref() {
        span.record("t", tracing::field::debug(t));
    }

    if let (Some(r), Some(w)) = (f, t) {    
        CopyPackets{
            r,
            w,
            first_poll: true,
            span,
            phase,
            flags,
            b,
        }.wrap()
    } else {
        warn!(parent: &span, "Nothing to copy");
        Arc::new(Mutex::new(None))
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("copy_bytes", copy_bytes);
    engine.register_fn("copy_bytes_bidirectional", copy_bytes_bidirectional);
    engine.register_fn("copy_packets", copy_packets);
}
