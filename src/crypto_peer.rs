use futures::Async;
use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io::Error as IoError;

use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::Nonce;
use chacha20poly1305::aead::NewAead;
use chacha20poly1305::aead::Aead;
use rand::RngCore;

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
[A] Encrypts written messages and decryptes (and verifies) read messages with a static key, using ChaCha20-Poly1305 algorithm.

Do not not use in stream mode - packet boundaries are significant.
"#
);

#[derive(Clone, Copy)]
enum Mode {
    Encrypt,
    Decrypt,
}

pub fn crypto_peer(inner_peer: Peer) -> BoxedNewPeerFuture {
    let key = [0u8; 32];
    let crypto = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key));
    let filtered_r = CryptoWrapperR(inner_peer.0, crypto.clone(), Mode::Decrypt);
    let filtered_w = CryptoWrapperW(inner_peer.1, crypto, Mode::Encrypt);
    let thepeer = Peer::new(filtered_r, filtered_w, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct CryptoWrapperR(Box<dyn AsyncRead>, ChaCha20Poly1305, Mode);

impl Read for CryptoWrapperR {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut l = b.len();

        assert!(l > 12);

        if matches!(self.2, Mode::Encrypt) {
            l -= 12;
        }

        let n = match self.0.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        if n == 0 { return Ok(0) }

        let data = process_data(&b[..n], &self.1, self.2)?;

        let m = data.len();
        b[..m].copy_from_slice(&data[..m]);

        Ok(m)
    }
}
impl AsyncRead for CryptoWrapperR {}

struct CryptoWrapperW(Box<dyn AsyncWrite>, ChaCha20Poly1305, Mode);

impl Write for CryptoWrapperW {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let l = b.len();

        let data = process_data(b, &self.1, self.2)?;

        let n = match self.0.write(&data[..]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };

        if n != data.len() {
            log::error!("Short write when using `crypto:` specifier");
        }

        Ok(l)
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

fn process_data(buf: &[u8], crypto: &ChaCha20Poly1305, mode: Mode) -> Result<Vec<u8>, IoError>  {
    let l = buf.len();
    match mode {
        Mode::Encrypt => {
            let mut nonce = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce[..]);
            let mut data: Vec<u8> = crypto
                .encrypt(Nonce::from_slice(&nonce), &buf[..])
                .unwrap();
            data.extend_from_slice(&nonce[..]);
            Ok(data)
        }
        Mode::Decrypt => {
            if l < 12 {
                log::error!("Insufficient packet length for `crypto:` specifier's decryption");
                return Err(std::io::ErrorKind::Other.into()); 
            }
            let mut nonce = [0u8; 12];
            nonce.copy_from_slice(&buf[l-12..l]);
            match crypto.decrypt(Nonce::from_slice(&nonce), &buf[..(l-12)]) {
                Ok(x) => Ok(x),
                Err(_) => {
                    log::error!("crypto: decryption failed");
                    return Err(std::io::ErrorKind::Other.into())
                }
            }
        }
    }
}
