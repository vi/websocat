use argon2::Argon2;
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
        let mut key = [0u8; 32];
        if let Some(k) = cp.program_options.crypto_key {
            key = k;
        } else {
            log::error!("You are using `crypto:` without `--crypto-key`. This uses a hard coded key and is insecure.")
        }
        inner.map(move |p, _| crypto_peer(p, key, cp.program_options.crypto_reverse))
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
[A] Encrypts written messages and decrypts (and verifies) read messages with a static key, using ChaCha20-Poly1305 algorithm.

Do not not use in stream mode - packet boundaries are significant.

Note that attacker may duplicate, drop or reorder messages, including between different Websocat sessions with the same key.

Each encrypted message is 12 bytes bigger than original message.

Associated --crypto-key option accepts the following prefixes:

- `file:` prefix means that Websocat should read 32-byte file and use it as a key.
- `base64:` prefix means the rest of the value is base64-encoded 32-byte buffer
- `pwd:` means Websocat should use argon2 derivation from the specified password as a key

Use `--crypto-reverse` option to swap encryption and decryption.

Note that `crypto:` specifier is absent in usual Websocat builds.
You may need to build Websocat from source code with `--features=crypto_peer` for it to be available.
"#
);

#[derive(Clone, Copy)]
enum Mode {
    Encrypt,
    Decrypt,
}

pub fn crypto_peer(inner_peer: Peer, key: [u8; 32], reverse: bool) -> BoxedNewPeerFuture {
    let (mode_r, mode_w) = if reverse {
        (Mode::Encrypt, Mode::Decrypt)
    } else {
        (Mode::Decrypt, Mode::Encrypt)
    };
    let crypto = ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key));
    let filtered_r = CryptoWrapperR(inner_peer.0, crypto.clone(), mode_r);
    let filtered_w = CryptoWrapperW(inner_peer.1, crypto, mode_w);
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

pub fn interpret_opt(x: &str) -> crate::Result<[u8; 32]> {
    let mut key = [0u8; 32];
    if x.starts_with("base64:") {
        let mut buf = Vec::with_capacity(32);
        base64::decode_config_buf(&x[7..], base64::STANDARD, &mut buf)?;
        if buf.len() != 32 {
            log::error!("Expected 32 bytes, got {} bytes", buf.len());
            return Err("Non 32-byte buffer specified".into());
        }
        key.copy_from_slice(&buf[..]);

    } else if x.starts_with("file:") {
        let buf = std::fs::read(&x[5..])?;
        if buf.len() != 32 {
            log::error!("Expected 32 bytes, got {} bytes", buf.len());
            return Err("Non 32-byte buffer specified".into());
        }
        key.copy_from_slice(&buf[..])
    } else if x.starts_with("pwd:") {
        let argon2 = Argon2::default();
        const SALT : &'static [u8] = &[0x81, 0x65, 0x0c, 0xc7, 0x09, 0x76, 0xc1, 0x12, 0x6b, 0x5b, 0x5f, 0x04,
        0x08, 0x61, 0xf6, 0x1b, 0xd6, 0xab, 0x88, 0xa2, 0xee, 0x67, 0x47, 0xc1,
        0xbe, 0x12, 0xd7, 0xd7, 0x2d, 0xb8, 0x39, 0xcf];
        argon2.hash_password_into(x[4..].as_bytes(),SALT,&mut key[..]).unwrap();
    } else {
        return Err("--crypto-key's value must start with `base64:`, `file:` or `pwd:`".into());
    }
    Ok(key)
}
