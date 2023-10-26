use futures::StreamExt;
use rhai::Engine;

use crate::types::{Handle, StreamWrite, StreamRead, TaskHandleExt, Task, DatagramStream, DatagramSink, Buffer};

fn copydata(from: Handle<StreamRead>, to: Handle<StreamWrite>) -> Handle<Task> {
    async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());

        if let (Some(mut r), Some(mut w)) = (f, t) {
            eprintln!(
                "copy read={:?} write={:?}",
                &*r as *const _,
                &*w as *const _,
            );

            match tokio::io::copy(&mut r, &mut w).await {
                Ok(x) => eprintln!("Copied {x} bytes"),
                Err(e) => eprintln!("Error from copydata: {e}"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }.wrap()
}


fn copy_packets(from: Handle<DatagramStream>, to: Handle<DatagramSink>) -> Handle<Task> {
    async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());
        if let (Some(r), Some(w)) = (f, t) {
            *w.pool.lock().unwrap() = Some(r.pool.clone());
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
    engine.register_fn("copydata", copydata);
    engine.register_fn("copy_packets", copy_packets);
}
