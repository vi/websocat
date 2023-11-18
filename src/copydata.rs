use futures::StreamExt;
use rhai::Engine;
use tokio::io::AsyncWriteExt;
use tracing::{debug_span, debug, field, Instrument};

use crate::types::{Handle, StreamWrite, StreamRead, TaskHandleExt, Task, DatagramStream, DatagramSink, Buffer, HandleExt2};

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


fn copy_packets(from: Handle<DatagramStream>, to: Handle<DatagramSink>) -> Handle<Task> {
    async move {
        let (f, t) = (from.lut(), to.lut());
        if let (Some(r), Some(w)) = (f, t) {
            match r.src.map(|x|Ok::<Buffer,()>(x)).forward(w.snk).await {
                Ok(()) => eprintln!("Finished forwarding"),
                Err(()) => eprintln!("Error from copy_packets"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }.wrap()
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("copy_bytes", copy_bytes);
    engine.register_fn("copy_packets", copy_packets);
}
