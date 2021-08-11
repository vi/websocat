use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};
use std::time::{SystemTime, UNIX_EPOCH, Instant};

use std::io::Read;
use tokio_io::AsyncRead;

use std::io::Error as IoError;

#[derive(Debug)]
pub struct TimestampPeer<T: Specifier>(pub T);
impl<T: Specifier> Specifier for TimestampPeer<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _| timestamp_peer(p, cp.program_options.timestamp_monotonic))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = TimestampClass,
    target = TimestampPeer,
    prefixes = ["timestamp:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Prepend timestamp to each incoming message.

Example: TODO
"#
);

pub fn timestamp_peer(inner_peer: Peer, monotonic: bool) -> BoxedNewPeerFuture {
    let instant = if monotonic { Some(Instant::now() )} else { None };
    let filtered = TimestampWrapper(inner_peer.0, instant);
    let thepeer = Peer::new(filtered, inner_peer.1, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct TimestampWrapper(Box<dyn AsyncRead>, Option<Instant>);

impl Read for TimestampWrapper {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let l = b.len();
        assert!(l > 1);
        let n = match self.0.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(0);
        }

        let mut v: Vec<u8> = Vec::with_capacity(n + 50);
        {
            let mut vv = ::std::io::Cursor::new(&mut v);
            use std::io::Write;
            let x = if let Some(basetime) = self.1 {
                Instant::now().duration_since(basetime).as_secs_f64()
            } else {
                (SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards")).as_secs_f64()
            };
            let _ = write!(vv, "{} ", x);
            let _ = vv.write_all(&b[..n]);
        }
        
        if v.len() as usize > l {
            warn!("Buffer too small, timstamp-prepended message may be truncated.");
        }
        let ll = v.len().min(l);
        (&mut b[..ll]).copy_from_slice(&v[..ll]);
        Ok(ll)
    }
}
impl AsyncRead for TimestampWrapper {}
