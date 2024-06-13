use std::{pin::Pin, task::Poll};

use pin_project::pin_project;
use rhai::{Dynamic, Engine};
use tokio::io::ReadBuf;
use tracing::{debug, debug_span, error, field, Instrument};

use crate::scenario_executor::{
    debugfluff::PtrDbg,
    types::{BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, PacketRead, PacketWrite, StreamRead, StreamSocket, StreamWrite, Task},
    utils::{
        run_task, ExtractHandleOrFail, HandleExt, RhResult,
        TaskHandleExt,
    },
};

use super::types::PacketReadResult;

fn take_read_part(h: Handle<StreamSocket>) -> Handle<StreamRead> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        if let Some(hh) = s.read.take() {
            Some(hh).wrap()
        } else {
            None.wrap()
        }
    } else {
        None.wrap()
    }
}

fn take_write_part(h: Handle<StreamSocket>) -> Handle<StreamWrite> {
    if let Some(s) = h.lock().unwrap().as_mut() {
        if let Some(hh) = s.write.take() {
            Some(hh).wrap()
        } else {
            None.wrap()
        }
    } else {
        None.wrap()
    }
}
fn dummytask() -> Handle<Task> {
    async move {}.wrap_noerr()
}

fn sleep_ms(ms: i64) -> Handle<Task> {
    async move { tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await }
        .wrap_noerr()
}

fn sequential(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        for t in tasks {
            let Some(t): Option<Handle<Task>> = t.try_cast() else {
                error!("Not a task in a list of tasks");
                continue;
            };
            run_task(t).await;
        }
    }
    .wrap_noerr()
}

fn parallel(tasks: Vec<Dynamic>) -> Handle<Task> {
    async move {
        let mut waitees = Vec::with_capacity(tasks.len());
        for t in tasks {
            let Some(t): Option<Handle<Task>> = t.try_cast() else {
                error!("Not a task in a list of tasks");
                continue;
            };
            waitees.push(tokio::spawn(run_task(t)));
        }
        for w in waitees {
            let _ = w.await;
        }
    }
    .wrap_noerr()
}

fn spawn_task(task: Handle<Task>) {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "spawn", t = field::Empty);
    if let Some(x) = task.lock().unwrap().as_ref() {
        span.record("t", tracing::field::debug(PtrDbg(&**x)));
        debug!(parent: &span, "Spawning");
    } else {
        error!("Attempt to spawn a null/taken task");
    }
    tokio::spawn(
        async move {
            run_task(task).await;
            debug!("Finished");
        }
        .instrument(span),
    );
}

#[pin_project]
struct ReadStreamChunks(StreamRead);

impl PacketRead for ReadStreamChunks {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let sr: &mut StreamRead = self.project().0;

        if !sr.prefix.is_empty() {
            let limit = buf.len().min(sr.prefix.len());
            buf[0..limit].copy_from_slice(&sr.prefix.split_to(limit));
            return Poll::Ready(Ok(PacketReadResult {
                flags: BufferFlags::default(),
                buffer_subset: 0..limit,
            }));
        }

        let mut rb = ReadBuf::new(buf);
        
        match tokio::io::AsyncRead::poll_read(sr.reader.as_mut(), cx, &mut rb) {
            Poll::Ready(Ok(())) => {
                let new_len = rb.filled().len();
                let flags = if new_len > 0 {
                    BufferFlags::default()
                } else {
                    BufferFlag::Eof.into()
                };
                Poll::Ready(Ok(PacketReadResult {
                    flags,
                    buffer_subset: 0..new_len,
                }))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn read_stream_chunks(x: Handle<StreamRead>) -> RhResult<Handle<DatagramRead>> {
    let x = x.lutbar()?;
    debug!(inner=?x, "read_stream_chunks");
    let x = DatagramRead {
        src: Box::pin(ReadStreamChunks(x)),
    };
    debug!(wrapped=?x, "read_stream_chunks");
    Ok(x.wrap())
}

#[pin_project]
struct WriteStreamChunks { w: StreamWrite, debt: usize }

impl PacketWrite for WriteStreamChunks {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let p = self.project();
        let sw: &mut StreamWrite = p.w;
        
        loop {
            assert!(buf.len() >= *p.debt);
            let buf_chunk = &buf[*p.debt..];
            if buf_chunk.is_empty() {
                if !flags.contains(BufferFlag::NonFinalChunk) {
                    match tokio::io::AsyncWrite::poll_flush(sw.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                if flags.contains(BufferFlag::Eof) {
                    match tokio::io::AsyncWrite::poll_shutdown(sw.writer.as_mut(), cx) {
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                *p.debt = 0;
                break;
            }
            match tokio::io::AsyncWrite::poll_write(sw.writer.as_mut(), cx, buf_chunk) {
                Poll::Ready(Ok(n)) => {
                    *p.debt += n;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        return Poll::Ready(Ok(()))
    }
}

fn write_stream_chunks(x: Handle<StreamWrite>) -> RhResult<Handle<DatagramWrite>> {
    let x = x.lutbar()?;
    debug!(inner=?x, "write_stream_chunks");
    let x = DatagramWrite {
        snk: Box::pin(WriteStreamChunks{w:x, debt:0}),
    };
    debug!(wrapped=?x, "write_stream_chunks");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("dummy_task", dummytask);
    engine.register_fn("sleep_ms", sleep_ms);
    engine.register_fn("sequential", sequential);
    engine.register_fn("parallel", parallel);
    engine.register_fn("spawn_task", spawn_task);
    engine.register_fn("read_stream_chunks", read_stream_chunks);
    engine.register_fn("write_stream_chunks", write_stream_chunks);
}
