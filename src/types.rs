use std::{sync::{Arc,Mutex}, pin::Pin};

use bytes::BytesMut;
use futures::{Future, Stream, Sink};
use object_pool::Pool;
use tokio::io::{AsyncRead, AsyncWrite};
pub type Handle<T> = Arc<Mutex<Option<T>>>;

pub trait HandleExt {
    type HandleInner;
    fn wrap(self) -> Handle<Self::HandleInner>;
}

impl<T> HandleExt for Option<T> {
    type HandleInner = T;
    fn wrap(self) -> Handle<T> {    
        Arc::new(Mutex::new(self))
    }
}

pub type Task = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type Hangup = Pin<Box<dyn Future<Output = ()> + Send>>;

pub trait TaskHandleExt {
    fn wrap(self) -> Handle<Task>;
}

impl<T : Future<Output = ()> + Send + 'static > TaskHandleExt for T {
    fn wrap(self) -> Handle<Task> {
        Arc::new(Mutex::new(Some(Box::pin(self))))
    }
}

pub async fn run_task(h: Handle<Task>) {
    let Some(t) = h.lock().unwrap().take() else {
        eprintln!("No task requested");
        return;
    };
    t.await
}

pub type StreamRead = Pin<Box<dyn AsyncRead + Send>>;
pub type StreamWrite = Pin<Box<dyn AsyncWrite + Send>>;

pub struct StreamSocket {
    pub read: Option<StreamRead>,
    pub write: Option<StreamWrite>,
    pub close: Option<Hangup>,
}

impl StreamSocket {
    pub fn wrap(self) -> Handle<StreamSocket> {
        Arc::new(Mutex::new(Some(self)))
    }
}

pub struct Buffer {
    pub data: BytesMut,
}
impl Buffer {
    pub fn new() -> Buffer {
        Buffer { data: BytesMut::new() }
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }
}
pub type BufferPool = Arc<Pool<Buffer>>;

pub struct DatagramStream {
    pub src: Pin<Box<dyn Stream<Item = Buffer> + Send>>,
    pub pool: BufferPool,
}
pub struct DatagramSink {
    pub snk: Pin<Box<dyn Sink<Buffer, Error = ()> + Send>>,
    pub pool: Handle<BufferPool>,
}
