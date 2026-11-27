use std::{
    io::ErrorKind,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{ready, Context, Poll},
    time::Duration,
};

use rhai::{Dynamic, Engine, NativeCallContext};
use std::io::Write;
use tokio::{sync::OwnedSemaphorePermit, time::Instant};
use tokio_util::sync::PollSemaphore;
use tracing::{debug, debug_span, warn, Instrument};

use crate::scenario_executor::{
    scenario::{Scenario, ScenarioAccess},
    utils1::{ExtractHandleOrFail, SimpleErr},
    utils2::PollSemaphoreNew2,
};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead,
        PacketReadResult, PacketWrite,
    },
    utils1::RhResult,
};

/// Same format as in Websocat1: seconds (u64 BE) and nanoseconds (u32 BE) since the pinger was created.
const PING_PAYLOAD_LEN: usize = 12;

/// Longest control message payload allowed by WebSocket.
const MAX_PONG_LEN: usize = 125;

struct SharedWriter {
    inner: Pin<Box<dyn PacketWrite + Send>>,
    sem: PollSemaphore,
    eof_written: bool,
}

/// Writes from the main data path. Holds the semaphore permit while a write is pending,
/// so pings cannot be injected into the middle of a buffer.
struct DataWriter {
    shared: Arc<Mutex<SharedWriter>>,
    sem_permit: Option<OwnedSemaphorePermit>,
}

impl PacketWrite for DataWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut shared = this.shared.lock().unwrap();

        if this.sem_permit.is_none() {
            match ready!(shared.sem.poll_acquire(cx)) {
                None => return Poll::Ready(Err(ErrorKind::ConnectionReset.into())),
                Some(p) => this.sem_permit = Some(p),
            }
        }

        let ret = ready!(shared.inner.as_mut().poll_write(cx, buf, flags));
        if flags.contains(BufferFlag::Eof) {
            shared.eof_written = true;
        }
        this.sem_permit = None;
        Poll::Ready(ret)
    }
}

/// Passes everything through, but measures round-trip time when a Pong to our ping comes back.
struct PongWatcher {
    inner: Pin<Box<dyn PacketRead + Send>>,
    baseline: Instant,
    pong_buf: Vec<u8>,
    print_rtts: bool,
    scenario: Weak<Scenario>,
}

impl PongWatcher {
    fn handle_pong(&mut self) {
        let Ok(payload) = <[u8; PING_PAYLOAD_LEN]>::try_from(&self.pong_buf[..]) else {
            debug!("pong with a foreign payload");
            return;
        };
        let secs = u64::from_be_bytes(payload[0..8].try_into().unwrap());
        let nanos = u32::from_be_bytes(payload[8..12].try_into().unwrap());
        if nanos >= 1_000_000_000 {
            debug!("pong with a foreign payload");
            return;
        }
        let sent = Duration::new(secs, nanos);
        let rtt = self
            .baseline
            .elapsed()
            .checked_sub(sent)
            .unwrap_or_default();
        debug!(?rtt, "received a pong");
        if self.print_rtts {
            let Some(the_scenario) = self.scenario.upgrade() else {
                return;
            };
            let Ok(mut diago) = the_scenario.diagnostic_output.lock() else {
                return;
            };
            let _ = writeln!(diago, "RTT {}.{:06} s", rtt.as_secs(), rtt.subsec_micros());
        }
    }
}

impl PacketRead for PongWatcher {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();
        let ret = ready!(this.inner.as_mut().poll_read(cx, buf))?;

        if ret.flags.contains(BufferFlag::Pong) {
            let chunk = &buf[ret.buffer_subset.clone()];
            if this.pong_buf.len() + chunk.len() <= MAX_PONG_LEN {
                this.pong_buf.extend_from_slice(chunk);
            }
            if !ret.flags.contains(BufferFlag::NonFinalChunk) {
                this.handle_pong();
                this.pong_buf.clear();
            }
        }

        Poll::Ready(Ok(ret))
    }
}

enum PingWriteOutcome {
    Sent,
    Stop,
}

async fn send_one_ping(
    shared: &Weak<Mutex<SharedWriter>>,
    payload: &mut [u8; PING_PAYLOAD_LEN],
) -> PingWriteOutcome {
    let mut sem_permit: Option<OwnedSemaphorePermit> = None;
    std::future::poll_fn(|cx| {
        let Some(shared) = shared.upgrade() else {
            return Poll::Ready(PingWriteOutcome::Stop);
        };
        let mut shared = shared.lock().unwrap();
        if shared.eof_written {
            return Poll::Ready(PingWriteOutcome::Stop);
        }
        if sem_permit.is_none() {
            match ready!(shared.sem.poll_acquire(cx)) {
                None => return Poll::Ready(PingWriteOutcome::Stop),
                Some(p) => sem_permit = Some(p),
            }
        }
        let ret =
            ready!(shared
                .inner
                .as_mut()
                .poll_write(cx, &mut payload[..], BufferFlag::Ping.into()));
        sem_permit = None;
        match ret {
            Ok(()) => Poll::Ready(PingWriteOutcome::Sent),
            Err(e) => {
                warn!("failed to send a ping: {e}");
                Poll::Ready(PingWriteOutcome::Stop)
            }
        }
    })
    .await
}

async fn ping_loop(
    shared: Weak<Mutex<SharedWriter>>,
    baseline: Instant,
    interval: Duration,
    max_pings: Option<usize>,
) {
    let mut timer = tokio::time::interval_at(baseline + interval, interval);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sent = 0usize;
    loop {
        if max_pings.is_some_and(|m| sent >= m) {
            debug!("not sending pings anymore");
            return;
        }
        timer.tick().await;

        let ts = baseline.elapsed();
        let mut payload = [0u8; PING_PAYLOAD_LEN];
        payload[0..8].copy_from_slice(&ts.as_secs().to_be_bytes());
        payload[8..12].copy_from_slice(&ts.subsec_nanos().to_be_bytes());

        match send_one_ping(&shared, &mut payload).await {
            PingWriteOutcome::Sent => {
                debug!("sent a ping");
                sent += 1;
            }
            PingWriteOutcome::Stop => {
                debug!("socket is gone or shut down, stopping pings");
                return;
            }
        }
    }
}

/// Wrap the writer so that it can be shared with a ping timer task, then start the task.
fn start_pinging(
    inner: Pin<Box<dyn PacketWrite + Send>>,
    baseline: Instant,
    interval: Duration,
    max_pings: Option<usize>,
) -> DataWriter {
    let shared = Arc::new(Mutex::new(SharedWriter {
        inner,
        sem: PollSemaphore::new2(1),
        eof_written: false,
    }));
    tokio::spawn(
        ping_loop(Arc::downgrade(&shared), baseline, interval, max_pings).in_current_span(),
    );
    DataWriter {
        shared,
        sem_permit: None,
    }
}

//@ Periodically send pings to a datagram socket (e.g. WebSocket) and optionally measure round-trip times when pongs come back.
//@
//@ Pings are written as `Ping`-flagged buffers, so they become WebSocket ping frames after `ws_wrap`,
//@ or control messages in `lengthprefixed:` with control messages enabled.
//@ Ping payload is 12 bytes: seconds (u64) and nanoseconds (u32) since this function was called, big endian.
//@ Pongs and everything else are passed through to the reader unchanged.
//@
//@ This does not drop the connection if pongs stop coming.
fn ping_sender(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("ping_sender");
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Send a ping each this number of milliseconds
        interval_ms: u64,

        //@ Stop sending pings after this number of them
        max_pings: Option<usize>,

        //@ Print measured round-trip times to diagnostic output (stderr) after each pong
        #[serde(default)]
        print_rtts: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    if opts.interval_ms == 0 {
        return Err(ctx.err("interval_ms must not be zero"));
    }
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let DatagramSocket {
        read,
        write,
        close,
        fd,
    } = inner;

    let (Some(inner_read), Some(inner_write)) = (read, write) else {
        return Err(ctx.err("Incomplete datagram socket"));
    };

    let baseline = Instant::now();
    let _entered = span.enter();

    let writer = start_pinging(
        inner_write.snk,
        baseline,
        Duration::from_millis(opts.interval_ms),
        opts.max_pings,
    );

    let x = DatagramSocket {
        read: Some(DatagramRead {
            src: Box::pin(PongWatcher {
                inner: inner_read.src,
                baseline,
                pong_buf: Vec::new(),
                print_rtts: opts.print_rtts,
                scenario: Arc::downgrade(&ctx.get_scenario()?),
            }),
        }),
        write: Some(DatagramWrite {
            snk: Box::pin(writer),
        }),
        close,
        fd,
    };
    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ping_sender", ping_sender);
}

#[cfg(test)]
mod tests {
    use super::*;

    type Log = Arc<Mutex<Vec<(BufferFlags, Vec<u8>)>>>;

    struct Recorder(Log);

    impl PacketWrite for Recorder {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
            flags: BufferFlags,
        ) -> Poll<std::io::Result<()>> {
            self.0.lock().unwrap().push((flags, buf.to_vec()));
            Poll::Ready(Ok(()))
        }
    }

    async fn write(w: &mut DataWriter, buf: &[u8], flags: BufferFlags) {
        let mut buf = buf.to_vec();
        std::future::poll_fn(|cx| Pin::new(&mut *w).poll_write(cx, &mut buf, flags))
            .await
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn no_pings_after_eof() {
        let log = Log::default();
        let mut w = start_pinging(
            Box::pin(Recorder(log.clone())),
            Instant::now(),
            Duration::from_secs(1),
            None,
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;
        write(&mut w, b"", BufferFlag::Eof.into()).await;
        tokio::time::sleep(Duration::from_secs(3)).await;

        let log = log.lock().unwrap();
        let flags: Vec<BufferFlags> = log.iter().map(|x| x.0).collect();
        let expected: [BufferFlags; 2] = [BufferFlag::Ping.into(), BufferFlag::Eof.into()];
        assert_eq!(flags, expected);
        assert_eq!(log[0].1, [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_writer_is_dropped() {
        let log = Log::default();
        let w = start_pinging(
            Box::pin(Recorder(log.clone())),
            Instant::now(),
            Duration::from_secs(1),
            None,
        );
        tokio::time::sleep(Duration::from_millis(1500)).await;
        drop(w);
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(log.lock().unwrap().len(), 1);
    }
}
