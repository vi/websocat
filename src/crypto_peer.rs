use futures::Async;
use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io::Error as IoError;

#[derive(Debug)]
pub struct Crypto<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Crypto<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _| crypto_peer(p))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = CryptoClass,
    target = Crypto,
    prefixes = ["crypto:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Encrypts written messages and decryptes (and verifies) read messages with a static key, using ChaCha20-Poly1305 algorithm`.
"#
);

pub fn crypto_peer(inner_peer: Peer) -> BoxedNewPeerFuture {
    let filtered_r = CryptoWrapperR(inner_peer.0);
    let filtered_w = CryptoWrapperW(inner_peer.1);
    let thepeer = Peer::new(filtered_r, filtered_w, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct CryptoWrapperR(Box<dyn AsyncRead>);

impl Read for CryptoWrapperR {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let l = b.len();
        assert!(l > 1);
        let n = match self.0.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        Ok(n)
    }
}
impl AsyncRead for CryptoWrapperR {}

struct CryptoWrapperW(Box<dyn AsyncWrite>);

impl Write for CryptoWrapperW {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let l = b.len();
        assert!(l > 1);
        let n = match self.0.write(&b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
impl AsyncWrite for CryptoWrapperW {
    fn shutdown(&mut self) -> std::result::Result<Async<()>, std::io::Error> {
        self.0.shutdown()
    }
}
