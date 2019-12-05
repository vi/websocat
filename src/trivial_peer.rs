use super::{BoxedNewPeerFuture, Peer};

use futures;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};

use futures::Async::Ready;

use std::rc::Rc;
use tokio_io::{AsyncRead, AsyncWrite};

use super::readdebt::{DebtHandling, ReadDebt};
use super::wouldblock;

use super::{once, simple_err, ConstructParams, PeerConstructor, Specifier};

#[derive(Clone)]
pub struct Literal(pub Vec<u8>);
impl Specifier for Literal {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(get_literal_peer(self.0.clone()))
    }
    specifier_boilerplate!(singleconnect no_subspec noglobalstate);
}
impl std::fmt::Debug for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "Literal")
    }
}
specifier_class!(
    name = LiteralClass,
    target = Literal,
    prefixes = ["literal:"],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Output a string, discard input.

Example:

    websocat ws-l:127.0.0.1:8080 literal:'{ "hello":"world"} '
"#
);
// TODO: better doc

#[derive(Clone)]
pub struct Assert(pub Vec<u8>);
impl Specifier for Assert {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(get_assert_peer(self.0.clone()))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
impl std::fmt::Debug for Assert {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "Assert")
    }
}
specifier_class!(
    name = AssertClass,
    target = Assert,
    prefixes = ["assert:"],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Check the input.  [A]

Read entire input and panic the program if the input is not equal
to the specified string. Used in tests.
"#
);

#[derive(Clone)]
pub struct Assert2(pub Vec<u8>);
impl Specifier for Assert2 {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(get_assert2_peer(self.0.clone()))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
impl std::fmt::Debug for Assert2 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "Assert2")
    }
}
specifier_class!(
    name = Assert2Class,
    target = Assert2,
    prefixes = ["assert2:"],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Check the input. [A]

Read entire input and emit an error if the input is not equal
to the specified string.
"#
);

#[derive(Debug, Clone)]
pub struct Clogged;
impl Specifier for Clogged {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(get_clogged_peer())
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = CloggedClass,
    target = Clogged,
    prefixes = ["clogged:"],
    arg_handling = noarg,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Do nothing. Don't read or write any bytes. Keep connections in "hung" state. [A]
"#
);

pub struct LiteralPeer {
    debt: ReadDebt,
}

pub fn get_literal_peer_now(b: Vec<u8>) -> LiteralPeer {
    LiteralPeer {
        debt: ReadDebt(Some(b), DebtHandling::Silent),
    }
}

pub fn get_literal_peer(b: Vec<u8>) -> BoxedNewPeerFuture {
    let r = get_literal_peer_now(b);
    let w = DevNull;
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
pub fn get_assert_peer(b: Vec<u8>) -> BoxedNewPeerFuture {
    let r = DevNull;
    let w = AssertPeer(vec![], b, true);
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
pub fn get_assert2_peer(b: Vec<u8>) -> BoxedNewPeerFuture {
    let r = DevNull;
    let w = AssertPeer(vec![], b, false);
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
/// A special peer that returns NotReady without registering for any wakeup, deliberately hanging all connections forever.
pub fn get_clogged_peer() -> BoxedNewPeerFuture {
    let r = CloggedPeer;
    let w = CloggedPeer;
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}

impl AsyncRead for LiteralPeer {}

impl Read for LiteralPeer {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            debug!("LiteralPeer debt");
            return ret;
        }
        debug!("LiteralPeer finished");
        Ok(0)
    }
}

pub struct DevNull;

impl AsyncWrite for DevNull {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(Ready(()))
    }
}
impl Write for DevNull {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}
impl AsyncRead for DevNull {}
impl Read for DevNull {
    fn read(&mut self, _buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        Ok(0)
    }
}

struct AssertPeer(Vec<u8>, Vec<u8>, bool);
impl AsyncWrite for AssertPeer {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        if self.2 {
            assert_eq!(self.0, self.1);
        } else if self.0 != self.1 {
            error!("Assertion failed");
            return Err(simple_err("Assertion failed".into()));
        }
        info!("Assertion succeed");
        Ok(Ready(()))
    }
}

impl Write for AssertPeer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

struct CloggedPeer;
impl AsyncWrite for CloggedPeer {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        wouldblock()
    }
}
impl Write for CloggedPeer {
    fn write(&mut self, _buf: &[u8]) -> IoResult<usize> {
        wouldblock()
    }
    fn flush(&mut self) -> IoResult<()> {
        wouldblock()
    }
}
impl AsyncRead for CloggedPeer {}
impl Read for CloggedPeer {
    fn read(&mut self, _buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        wouldblock()
    }
}

// TODO: make Prepend{Read,Write} available from command line

/// First read content of `header`, then start relaying from `inner`.
pub struct PrependRead {
    pub header: Vec<u8>,
    pub remaining: usize,
    pub inner: Box<dyn AsyncRead>,
}

impl AsyncRead for PrependRead {}

impl Read for PrependRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if self.remaining == 0 {
            trace!("PrependRead relay");
            return self.inner.read(buf);
        }
        let l = buf.len().min(self.remaining);
        debug!("PrependRead read debt {}", l);
        let offset = self.header.len() - self.remaining;
        buf[..l].copy_from_slice(&self.header[offset..(offset + l)]);
        let ret = l;
        self.remaining -= ret;
        if self.remaining == 0 {
            self.header.clear();
            self.header.shrink_to_fit();
        }
        Ok(l)
    }
}

/// First write `header` to `inner`, then start copying data directly to it.
pub struct PrependWrite {
    pub header: Vec<u8>,
    pub remaining: usize,
    pub inner: Box<dyn AsyncWrite>,
}

impl AsyncWrite for PrependWrite {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.inner.shutdown()
    }
}
impl Write for PrependWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        loop {
            if self.remaining == 0 {
                return self.inner.write(buf);
            }
            let offset = self.header.len() - self.remaining;
            let ret = self.inner.write(&self.header[offset..])?;
            self.remaining -= ret;
            if self.remaining == 0 {
                self.header.clear();
                self.header.shrink_to_fit();
            }
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
    }
}
