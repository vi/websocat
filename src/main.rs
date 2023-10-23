use std::{
    future::Future,
    sync::{Arc, Mutex}, pin::Pin,
};

use rhai::Engine;
use tokio::io::{AsyncRead, AsyncWrite};

pub type StreamRead  = Option<Pin<Box<dyn AsyncRead + Send>>>;
pub type StreamWrite = Option<Pin<Box<dyn AsyncWrite + Send>>>;
pub type Hangup      = Option<Pin<Box<dyn Future<Output = ()> + Send>>>;
pub type Task        = Option<Pin<Box<dyn Future<Output = ()> + Send>>>;
pub struct StreamSocket {
    pub read: StreamRead,
    pub write: StreamWrite,
    pub close: Hangup,
}
pub type StreamReadHandle = Arc<Mutex<StreamRead>>;
pub type StreamWriteHandle = Arc<Mutex<StreamWrite>>;
pub type HangpHandle = Arc<Mutex<Hangup>>;
pub type TaskHangle = Arc<Mutex<Task>>;
pub type StreamSocketHandle = Arc<Mutex<StreamSocket>>;

fn create_stdio() -> StreamSocketHandle {
    Arc::new(Mutex::new(StreamSocket {
        read: Some(Box::pin(tokio::io::stdin())),
        write: Some(Box::pin(tokio::io::stdout())),
        close: None,
    }))
}

fn take_read_part(h: StreamSocketHandle) -> StreamReadHandle {
    if let Some(hh) = h.lock().unwrap().read.take() {
        Arc::new(Mutex::new(Some(hh)))
    } else {
        Arc::new(Mutex::new(None))
    }
}
fn take_write_part(h: StreamSocketHandle) -> StreamWriteHandle {
    if let Some(hh) = h.lock().unwrap().write.take() {
        Arc::new(Mutex::new(Some(hh)))
    } else {
        Arc::new(Mutex::new(None))
    }
}
fn copydata(from : StreamReadHandle, to: StreamWriteHandle) -> TaskHangle {
    Arc::new(Mutex::new(Some(Box::pin(async move {
        let (f, t) = (from.lock().unwrap().take(), to.lock().unwrap().take());
        if let (Some(mut r), Some(mut w)) =  (f,t) {
            match tokio::io::copy(&mut r, &mut w).await {
                Ok(x) => eprintln!("Copied {x} bytes"),
                Err(e) => eprintln!("Error from copydata: {e}"),
            }
        } else {
            eprintln!("Nothing to copydata");
        }
    }))))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let f = std::fs::read(std::env::args().nth(1).unwrap())?;


    let mut engine = Engine::RAW;

    engine.register_fn("create_stdio", create_stdio);
    engine.register_fn("take_read_part", take_read_part);
    engine.register_fn("take_write_part", take_write_part);
    engine.register_fn("copydata", copydata);


    let task: TaskHangle = engine.eval(std::str::from_utf8(&f[..])?)?;

    if let Some(t) = task.lock().unwrap().take() {
        t.await;
    } else {
        eprintln!("No task requested");
    }

    Ok(())
}
