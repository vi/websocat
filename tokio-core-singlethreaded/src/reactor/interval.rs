//! Support for creating futures that represent intervals.
//!
//! This module contains the `Interval` type which is a stream that will
//! resolve at a fixed intervals in future

use std::io;
use std::time::{Duration, Instant};

use futures::Poll;
use futures::Stream;
use tokio_timer::Interval as NewInterval;

use reactor::Handle;

/// A stream representing notifications at fixed interval
///
/// Intervals are created through the `Interval::new` or
/// `Interval::new_at` methods indicating when a first notification
/// should be triggered and when it will be repeated.
///
/// Note that timeouts are not intended for high resolution timers, but rather
/// they will likely fire some granularity after the exact instant that they're
/// otherwise indicated to fire at.
#[must_use = "streams do nothing unless polled"]
pub struct Interval {
    new: NewInterval
}

impl Interval {
    /// Creates a new interval which will fire at `dur` time into the future,
    /// and will repeat every `dur` interval after
    ///
    /// This function will return a future that will resolve to the actual
    /// interval object. The interval object itself is then a stream which will
    /// be set to fire at the specified intervals
    pub fn new(dur: Duration, handle: &Handle) -> io::Result<Interval> {
        Interval::new_at(Instant::now() + dur, dur, handle)
    }

    /// Creates a new interval which will fire at the time specified by `at`,
    /// and then will repeat every `dur` interval after
    ///
    /// This function will return a future that will resolve to the actual
    /// timeout object. The timeout object itself is then a future which will be
    /// set to fire at the specified point in the future.
    pub fn new_at(at: Instant, dur: Duration, handle: &Handle)
        -> io::Result<Interval>
    {
        Ok(Interval {
            new: handle.remote.timer_handle.interval(at, dur)
        })
    }
}

impl Stream for Interval {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<()>, io::Error> {
        self.new.poll()
            .map(|async| async.map(|option| option.map(|_| ())))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }
}
