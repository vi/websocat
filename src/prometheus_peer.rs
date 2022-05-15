use futures::{Async, Future, Stream};
use futures::future::ok;

use std::cell::RefCell;
use std::net::{SocketAddr, TcpListener};
use std::rc::Rc;
use std::time::Duration;

use crate::ws_server_peer::http_serve::get_static_file_reply;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io::Error as IoError;

use prometheus::{IntCounter, Encoder};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "websocat")]
pub struct GlobalStats {
    /// TODO
    w_msgs: IntCounter,
    /// TODO
    r_msgs: IntCounter,

    /// TODO
    w_bytes: IntCounter,
    /// TODO
    r_bytes: IntCounter,

    /// TODO
    #[metric(buckets(0.1, 1, 10, 60, 300, 3600))]
    requests_duration_seconds: prometheus::Histogram,
}

pub type HGlobalStats = Rc<GlobalStats>;

pub type GlobalState = HGlobalStats;


pub fn new_global_stats() -> GlobalState {
    dbg!();
    Rc::new(GlobalStats::new(prometheus::default_registry()).unwrap())
}

pub fn serve(psa: SocketAddr) -> crate::Result<()> {
    let tcp = TcpListener::bind(&psa)?;
    debug!("Listening TCP socket for Prometheus metrics");

    std::thread::spawn(move || {
        for s in tcp.incoming() {
            if let Ok(mut s) = s {
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
"#
);


pub fn prometheus_peer(inner_peer: Peer, stats: HGlobalStats) -> BoxedNewPeerFuture {
    let filtered_r = StatsWrapperR(inner_peer.0, );
    let filtered_w = StatsWrapperW(inner_peer.1, );
    let thepeer = Peer::new(filtered_r, filtered_w, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct StatsWrapperR(Box<dyn AsyncRead>);

impl Read for StatsWrapperR {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut l = b.len();

        let n = match self.0.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        Ok(n)
    }
}
impl AsyncRead for StatsWrapperR {}

struct StatsWrapperW(Box<dyn AsyncWrite>);

impl Write for StatsWrapperW {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let l = b.len();

        let n = match self.0.write(b) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        Ok(n)
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

