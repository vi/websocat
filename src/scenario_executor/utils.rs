use futures::Future;
use rhai::{EvalAltResult, NativeCallContext};
use tokio::io::AsyncRead;
use tracing::{error, trace};

use crate::scenario_executor::types::{DatagramRead, DatagramWrite, Handle, StreamSocket, Task};
use std::{
    sync::{Arc, Mutex},
    task::Poll,
};

use super::{types::{DatagramSocket, StreamRead, StreamWrite}, wsupgrade::OutgoingResponse};

pub trait TaskHandleExt {
    fn wrap_noerr(self) -> Handle<Task>;
}
pub trait TaskHandleExt2 {
    fn wrap(self) -> Handle<Task>;
}

impl<T: Future<Output = ()> + Send + 'static> TaskHandleExt for T {
    fn wrap_noerr(self) -> Handle<Task> {
        use futures::FutureExt;
        Arc::new(Mutex::new(Some(Box::pin(self.map(|_| Ok(()))))))
    }
}
impl<T: Future<Output = anyhow::Result<()>> + Send + 'static> TaskHandleExt2 for T {
    fn wrap(self) -> Handle<Task> {
        Arc::new(Mutex::new(Some(Box::pin(self))))
    }
}

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

pub async fn run_task(h: Handle<Task>) {
    let Some(t) = h.lock().unwrap().take() else {
        error!("Attempt to run a null/taken task");
        return;
    };
    if let Err(e) = t.await {
        error!("{e}");
    }
}

impl StreamSocket {
    pub fn wrap(self) -> Handle<StreamSocket> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramRead {
    pub fn wrap(self) -> Handle<DatagramRead> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramWrite {
    pub fn wrap(self) -> Handle<DatagramWrite> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl StreamRead {
    pub fn wrap(self) -> Handle<StreamRead> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl StreamWrite {
    pub fn wrap(self) -> Handle<StreamWrite> {
        Arc::new(Mutex::new(Some(self)))
    }
}
impl DatagramSocket {
    pub fn wrap(self) -> Handle<DatagramSocket> {
        Arc::new(Mutex::new(Some(self)))
    }
}

/*
pub trait Anyhow2EvalAltResult<T> {
    fn tbar(self) -> Result<T, Box<EvalAltResult>>;
}
impl<T> Anyhow2EvalAltResult<T> for anyhow::Result<T> {
    fn tbar(self) -> Result<T, Box<EvalAltResult>> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Box::new(EvalAltResult::ErrorRuntime(
                rhai::Dynamic::from(format!("{e}")),
                rhai::Position::NONE,
            ))),
        }
    }
}
*/
pub trait ExtractHandleOrFail {
    /// Lock mutex, Unwrapping possible poison error, Take the thing from option contained inside, fail if is is none and convert the error to BoxAltResult.
    fn lutbar<T>(&self, h: Handle<T>) -> Result<T, Box<EvalAltResult>>;
}
impl ExtractHandleOrFail for NativeCallContext<'_> {
    fn lutbar<T>(&self, h: Handle<T>) -> Result<T, Box<EvalAltResult>> {
        match h.lut() {
            Some(x) => Ok(x),
            None => Err(self.err("Null handle")),
        }
    }
}

pub type RhResult<T> = Result<T, Box<EvalAltResult>>;

impl AsyncRead for StreamRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let sr = self.get_mut();

        if !sr.prefix.is_empty() {
            let limit = buf.remaining().min(sr.prefix.len());
            trace!(nbytes = limit, "Serving from prefix");
            buf.put_slice(&sr.prefix.split_to(limit));
            return Poll::Ready(Ok(()));
        }

        sr.reader.as_mut().poll_read(cx, buf)
    }
}

pub trait SimpleErr {
    fn err(&self, v: impl Into<rhai::Dynamic>) -> Box<EvalAltResult>;
}
impl SimpleErr for NativeCallContext<'_> {
    fn err(&self, v: impl Into<rhai::Dynamic>) -> Box<EvalAltResult> {
        Box::new(EvalAltResult::ErrorRuntime(v.into(), self.position()))
    }
}
