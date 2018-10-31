//! Support for creating futures that represent timeouts.
//!
//! This module contains the `Timeout` type which is a future that will resolve
//! at a particular point in the future.

use std::io;
use std::time::{Duration, Instant};

use futures::{Future, Poll};
use tokio_timer::Delay;

use reactor::Handle;

/// A future representing the notification that a timeout has occurred.
///
/// Timeouts are created through the `Timeout::new` or
/// `Timeout::new_at` methods indicating when a timeout should fire at.
/// Note that timeouts are not intended for high resolution timers, but rather
/// they will likely fire some granularity after the exact instant that they're
/// otherwise indicated to fire at.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Timeout {
    delay: Delay
}

impl Timeout {
    /// Creates a new timeout which will fire at `dur` time into the future.
    ///
    /// This function will return a Result with the actual timeout object or an
    /// error. The timeout object itself is then a future which will be
    /// set to fire at the specified point in the future.
    pub fn new(dur: Duration, handle: &Handle) -> io::Result<Timeout> {
        Timeout::new_at(Instant::now() + dur, handle)
    }

    /// Creates a new timeout which will fire at the time specified by `at`.
    ///
    /// This function will return a Result with the actual timeout object or an
    /// error. The timeout object itself is then a future which will be
    /// set to fire at the specified point in the future.
    pub fn new_at(at: Instant, handle: &Handle) -> io::Result<Timeout> {
        Ok(Timeout {
            delay: handle.remote.timer_handle.delay(at)
        })
    }

    /// Resets this timeout to an new timeout which will fire at the time
    /// specified by `at`.
    ///
    /// This method is usable even of this instance of `Timeout` has "already
    /// fired". That is, if this future has resolved, calling this method means
    /// that the future will still re-resolve at the specified instant.
    ///
    /// If `at` is in the past then this future will immediately be resolved
    /// (when `poll` is called).
    ///
    /// Note that if any task is currently blocked on this future then that task
    /// will be dropped. It is required to call `poll` again after this method
    /// has been called to ensure that a task is blocked on this future.
    pub fn reset(&mut self, at: Instant) {
        self.delay.reset(at)
    }
}

impl Future for Timeout {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        self.delay.poll()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }
}
