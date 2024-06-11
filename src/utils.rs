use futures::Future;
use tracing::error;

use crate::types::{Handle, StreamSocket, Task};
use std::sync::{Arc, Mutex};

pub trait TaskHandleExt {
    fn wrap_noerr(self) -> Handle<Task>;
}
pub trait TaskHandleExt2 {
    fn wrap(self) -> Handle<Task>;
}


impl<T: Future<Output = ()> + Send + 'static> TaskHandleExt for T {
    fn wrap_noerr(self) -> Handle<Task> {
        use futures::FutureExt;
        Arc::new(Mutex::new(Some(Box::pin(self.map(|_|Ok(()))))))
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
