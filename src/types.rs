use std::{sync::{Arc,Mutex}, pin::Pin};

use bytes::BytesMut;
use futures::{Future, Stream, Sink};
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

pub trait HandleExt2 {
    type Target;
    /// Lock, unwrap and take
    fn lut(&self) -> Self::Target;
}

impl<T> HandleExt2 for Handle<T> {
    type Target = Option<T>;
    fn lut(&self) -> Self::Target {
        self.lock().unwrap().take()
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


flagset::flags! {
    pub enum BufferFlag : u8 {
        Final,
        Text,
    }
}
pub type BufferFlags = flagset::FlagSet<BufferFlag>;

pub struct Buffer {
    pub data: BytesMut,
    pub flags: BufferFlags,
}
impl Buffer {
    
}


pub struct DatagramStream {
    pub src: Pin<Box<dyn Stream<Item = Buffer> + Send>>,
}
pub struct DatagramSink {
    pub snk: Pin<Box<dyn Sink<Buffer, Error = ()> + Send>>,
}
