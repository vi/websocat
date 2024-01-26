use std::{task::{Context, Poll}, sync::{Mutex, Arc}};

use rhai::Engine;
use tokio::io::{AsyncWriteExt, ReadBuf};
use tracing::{debug_span, debug, field, Instrument, error, info, warn};

use crate::types::{Handle, StreamWrite, StreamRead, TaskHandleExt, Task, DatagramRead, DatagramWrite, HandleExt2, BufferFlag, BufferFlags};

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
    engine.register_fn("copy_packets", copy_packets);
}
