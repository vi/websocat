use std::sync::{
    atomic::{AtomicI32, Ordering::Relaxed},
    Arc,
};
use tracing::info;

pub const EXIT_CODE_WEBSOCKET_UPGRADE_ERROR_NONWS: i32 = 3;
pub const EXIT_CODE_WEBSOCKET_UPGRADE_ERROR_BROKEN: i32 = 4;
pub const EXIT_CODE_TLS_CLIENT_FAIL: i32 = 5;


#[derive(Debug,Clone)]
pub struct ExitCodeTracker(Arc<AtomicI32>);

impl ExitCodeTracker {
    pub fn new() -> Self {
        ExitCodeTracker(Arc::new(AtomicI32::new(0)))
    }
    pub fn get(&self) -> i32 {
        self.0.load(Relaxed)
    }

    pub fn set(&self, code: i32) {
        if let Ok(old) = self
            .0
            .fetch_update(Relaxed, Relaxed, |old| Some(old.max(code)))
        {
            info!("Setting pending exit code to {code} (was {old})");
        }
    }
}
