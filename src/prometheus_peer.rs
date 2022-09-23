use futures::future::ok;
use futures::{Async};
use prometheus::core::{AtomicU64, Atomic};

use std::cell::RefCell;
use std::net::{SocketAddr, TcpListener};
use std::rc::Rc;
use std::time::Duration;

use crate::ws_server_peer::http_serve::get_static_file_reply;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::{Read, Write, ErrorKind};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io::Error as IoError;

use prometheus::{Encoder, IntCounter, Histogram};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "websocat1")]
pub struct GlobalStats {
    /// Number of times write function was called
    w_msgs: IntCounter,
    /// Number of times read function was called
    r_msgs: IntCounter,

    /// Total number of written bytes to the `prometheus:` node
    w_bytes: IntCounter,
    /// Total number of read bytes from the `prometheus:` node
    r_bytes: IntCounter,

    /// Number of times `prometheus:` overlay was instantiated
    connects: IntCounter,

    /// Number of times `prometheus:` overlay's destructor was called
    disconnects: IntCounter,

    /// Distribution of times between `prometheus:` overlay initiation and destruction
    #[metric(buckets(0.1, 1, 10, 60, 300, 3600))]
    session_durations: Histogram,

    /// Distribution of times between one `prometheus:` overlay initiation and the next initiation
    #[metric(buckets(0.1, 1, 10, 60, 300, 3600))]
    between_connects: Histogram,

    /// Distribution of the number of total bytes written to the overlay per connection 
    #[metric(buckets(0, 32, 1024, 65536, 1048576, 33554432, 1073741824))]
    session_w_bytes: Histogram,

    /// Distribution of the number of total bytes read to the overlay per connection 
    #[metric(buckets(0, 32, 1024, 65536, 1048576, 33554432, 1073741824))]
    session_r_bytes: Histogram,

    /// Distribution of the number of total count of write function calls to the overlay per connection 
    #[metric(buckets(0, 1, 2, 8, 64, 2048, 65536, 2097152))]
    session_w_msgs: Histogram,

    /// Distribution of the number of total count of read function calls to the overlay per connection 
    #[metric(buckets(0, 1, 2, 8, 64, 2048, 65536, 2097152))]
    session_r_msgs: Histogram,

    /// Distribution of the `session_r_bytes` divided by `session_durations` values
    #[metric(buckets(0.5, 10, 100, 1000, 1000_0, 1000_00, 1000_000, 10_000_000, 100_000_000))]
    session_avg_r_bps: Histogram,

    /// Distribution of the `session_w_bytes` divided by `session_durations` values
    #[metric(buckets(0.5, 10, 100, 1000, 1000_0, 1000_00, 1000_000, 10_000_000, 100_000_000))]
    session_avg_w_bps: Histogram,

    /// Distribution of byte lengths underlying `read` function successfully returned
    #[metric(buckets(0, 1, 10, 50, 300, 1024, 8192, 65536, 4194304))]
    read_lengths: Histogram,

    /// Number of times `read` function of the underlying specifier returned error (besides EAGAIN)
    read_errors: IntCounter,

    /// Number of times `read` function of the underlying specifier returned EAGAIN
    read_wouldblocks: IntCounter,

    /// Number of times `write` function of the underlying specifier returned error (besides EAGAIN)
    write_errors: IntCounter,

    /// Number of times `write` function of the underlying specifier returned EAGAIN
    write_wouldblocks: IntCounter,

    /// Distribution of byte lengths underlying `write` function successfully returned
    #[metric(buckets(0, 1, 10, 50, 300, 1024, 8192, 65536, 4194304))]
    write_lengths: Histogram,

    /// durations it took to make a function call to underlying node for reading
    #[metric(buckets(0.1e-3,1e-3,0.01,0.1,1,10))]
    read_timings: Histogram,

    /// durations it took to make a function call to underlying node for writing
    #[metric(buckets(0.1e-3,1e-3,0.01,0.1,1,10))]
    write_timings: Histogram,
}

pub type HGlobalStats = Rc<GlobalStats>;

pub type GlobalState = (HGlobalStats, Rc<RefCell<Option<prometheus::HistogramTimer>>>);

struct Droppie {
    w_msgs: AtomicU64,
    r_msgs: AtomicU64,
    w_bytes: AtomicU64,
    r_bytes: AtomicU64,
    session_timing: Option<prometheus::HistogramTimer>,
    handle: HGlobalStats,
}

impl Droppie {
    fn new(handle: HGlobalStats) -> Droppie {
        handle.connects.inc();
        Droppie {
            session_timing: Some(handle.session_durations.start_timer()),
            handle: handle,
            w_msgs: AtomicU64::new(0),
            r_msgs: AtomicU64::new(0),
            w_bytes: AtomicU64::new(0),
            r_bytes: AtomicU64::new(0),
        }
    }
}

impl Drop for Droppie {
    fn drop(&mut self) {
        let t = self.session_timing.take().unwrap().stop_and_record();
        self.handle.session_r_bytes.observe(self.r_bytes.get() as f64);
        self.handle.session_w_bytes.observe(self.w_bytes.get() as f64);
        self.handle.session_r_msgs.observe(self.r_msgs.get() as f64);
        self.handle.session_w_msgs.observe(self.w_msgs.get() as f64);
        let r_avg_bps = self.r_bytes.get() as f64 / t;
        let w_avg_bps = self.w_bytes.get() as f64 / t;
        self.handle.session_avg_r_bps.observe(r_avg_bps);
        self.handle.session_avg_w_bps.observe(w_avg_bps);
        self.handle.disconnects.inc();

    }
}

pub fn new_global_stats() -> GlobalState {
    (Rc::new(GlobalStats::new(prometheus::default_registry()).unwrap()), Rc::new(RefCell::new(None)))
}

pub fn serve(psa: SocketAddr) -> crate::Result<()> {
    let tcp = TcpListener::bind(&psa)?;
    debug!("Listening TCP socket for Prometheus metrics");

    std::thread::spawn(move || {
        for s in tcp.incoming() {
            if let Ok(s) = s {
                let mut s = std::io::BufWriter::new(s); 
                let stats = prometheus::default_registry().gather();
                let header = get_static_file_reply(None, "text/plain; version=0.0.4");
                let _ = s.write_all(&header[..]);
                let _ = prometheus::TextEncoder::default().encode(&stats[..], &mut s);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    Ok(())
}

#[derive(Debug)]
pub struct Prometheus<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Prometheus<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let stats: GlobalState = cp.global(new_global_stats).clone();
        let inner = self.0.construct(cp.clone());

        inner.map(move |p, _| prometheus_peer(p, stats.clone()))
    }
    specifier_boilerplate!(globalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = PrometheusClass,
    target = Prometheus,
    prefixes = ["prometheus:", "metrics:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Account connections, messages, bytes and other data and expose Prometheus metrics on a separate port.

Not included by default, build a crate with `--features=prometheus_peer` to have it.
You can also use `--features=prometheus_peer,prometheus/process` to have additional metrics.
"#
);

pub fn prometheus_peer(inner_peer: Peer, stats: GlobalState) -> BoxedNewPeerFuture {
    let droppie = Droppie::new(stats.0);

    // stops previous and start new timer
    *stats.1.borrow_mut() = Some(droppie.handle.between_connects.start_timer());

    let droppie = Rc::new(droppie);

    let r = StatsWrapperR(inner_peer.0, droppie.clone());
    let w = StatsWrapperW(inner_peer.1, droppie);
    let thepeer = Peer::new(r, w, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct StatsWrapperR(Box<dyn AsyncRead>, Rc<Droppie>);

impl Read for StatsWrapperR {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let timer = self.1.handle.read_timings.start_timer();
        let ret = self.0.read(b);
        timer.stop_and_record();
        match &ret {
            Ok(x) => {
                self.1.handle.read_lengths.observe(*x as f64);
                self.1.handle.r_msgs.inc();
                self.1.handle.r_bytes.inc_by(*x as u64);
                self.1.r_msgs.inc_by(1);
                self.1.r_bytes.inc_by(*x as u64);
            },
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                self.1.handle.read_wouldblocks.inc();
            },
            Err(_) => {
                self.1.handle.read_errors.inc();
            },
        };

        ret
    }
}
impl AsyncRead for StatsWrapperR {}

struct StatsWrapperW(Box<dyn AsyncWrite>, Rc<Droppie>);

impl Write for StatsWrapperW {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let timer = self.1.handle.write_timings.start_timer();
        let ret = self.0.write(b);
        timer.stop_and_record();
        
        match &ret {
            Ok(x) => {
                self.1.handle.write_lengths.observe(*x as f64);
                self.1.handle.w_msgs.inc();
                self.1.handle.w_bytes.inc_by(*x as u64);
                self.1.w_msgs.inc_by(1);
                self.1.w_bytes.inc_by(*x as u64);
            },
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                self.1.handle.write_wouldblocks.inc();
            },
            Err(_) => {
                self.1.handle.write_errors.inc();
            },
        };

        ret
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
impl AsyncWrite for StatsWrapperW {
    fn shutdown(&mut self) -> std::result::Result<Async<()>, std::io::Error> {
        self.0.shutdown()
    }
}
