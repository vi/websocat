use std::sync::{
    atomic::{AtomicI32, Ordering::Relaxed},
    Arc,
};
use tracing::debug;

pub const EXIT_CODE_WEBSOCKET_FRAMING: i32 = 3;
pub const EXIT_CODE_WEBSOCKET_UPGRADE_ERROR_NONWS: i32 = 5;
pub const EXIT_CODE_WEBSOCKET_UPGRADE_ERROR_BROKEN: i32 = 6;
pub const EXIT_CODE_TLS_CLIENT_FAIL: i32 = 8;
pub const EXIT_CODE_TCP_CONNECT_FAIL: i32 = 14;
pub const EXIT_CODE_HOSTNAME_LOOKUP_NO_IPS: i32 = 20;
pub const EXIT_CODE_HOSTNAME_LOOKUP_FAIL: i32 = 21;

#[derive(Debug, Clone)]
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
            if code < old {
                debug!("Leaving exit code on {old} despite of attempt to set it to {code}");
            } else {
                debug!("Setting pending exit code to {code} (was {old})");
            }
        }
    }
}
