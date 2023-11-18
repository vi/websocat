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

pub struct StreamRead {
    pub reader: Pin<Box<dyn AsyncRead + Send>>,
    pub prefix: BytesMut,
}
pub struct StreamWrite {
    pub writer: Pin<Box<dyn AsyncWrite + Send>>,
} 

impl std::fmt::Debug for StreamRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"SR")?;
        if !self.prefix.is_empty() {
            write!(f, "{{{}}}", self.prefix.len())?;
        }
        write!(f, "@{:p}", self.reader)
    }
}
impl std::fmt::Debug for StreamWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SW@{:p}", self.writer)
    }
}

pub struct StreamSocket {
    pub read: Option<StreamRead>,
    pub write: Option<StreamWrite>,
    pub close: Option<Hangup>,
}

impl std::fmt::Debug for StreamSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SS(")?;
        if let Some(ref r) = self.read {
            r.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(ref w) = self.write {
            w.fmt(f)?;
        }
        write!(f, ",")?;
        if let Some(_) = self.close {
            write!(f, "H")?;
        }
        write!(f, ")")?;
        Ok(())
    }
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
