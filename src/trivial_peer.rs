use super::{BoxedNewPeerFuture, Peer};

use futures;
use rand::RngCore;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};

use futures::Async::Ready;

use std::rc::Rc;
use tokio_io::{AsyncRead, AsyncWrite};

use super::readdebt::{DebtHandling, ReadDebt, ZeroMessagesHandling};
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
        debt: ReadDebt(Some(b), DebtHandling::Silent, ZeroMessagesHandling::Deliver),
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

pub struct CloggedPeer;
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
        buf[..l].copy_from_slice(&self.header[offset..(offset+l)]);
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

#[derive(Debug)]
pub struct Log<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Log<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _l2r| {
            Box::new(futures::future::ok(Peer(Box::new(LogRead(p.0)), Box::new(LogWrite(p.1)), p.2)))
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = LogClass,
    target = Log,
    prefixes = ["log:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Log each buffer as it pass though the underlying connector.

If you increase the logging level, you will also see hex buffers.

Example: view WebSocket handshake and traffic on the way to echo.websocket.org

    websocat -t - ws-c:log:tcp:127.0.0.1:1080 --ws-c-uri ws://echo.websocket.org

"#
);

pub struct LogRead (pub Box<dyn AsyncRead>);

fn log_buffer(tag: &'static str, buf: &[u8]) {
    let mut s = String::with_capacity(buf.len()*2);
    for x in buf.iter().cloned().map(std::ascii::escape_default) {
        s.push_str(String::from_utf8_lossy(&x.collect::<Vec<u8>>()).as_ref() );
    }
    eprintln!("{} {} \"{}\"", tag, buf.len(), s );
    debug!("{}", hex::encode(buf));
}


impl AsyncRead for LogRead {}

impl Read for LogRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        let ret = self.0.read(buf);

        if let Ok(ref sz) = ret {
            let buf = &buf[..*sz];
            log_buffer("READ", buf);
        } else {
            //eprintln!("FAILED_READ");
        }

        ret
    }
}

pub struct LogWrite(pub Box<dyn AsyncWrite>);

impl AsyncWrite for LogWrite {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.0.shutdown()
    }
}
impl Write for LogWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        
        let ret = self.0.write(buf);

        if let Ok(ref sz) = ret {
            let buf = &buf[..*sz];
            log_buffer("WRITE", buf);
        } else {
            //eprintln!("FAILED_WRITE");
        }

        ret
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}


#[derive(Debug)]
pub struct Random;
impl Specifier for Random {
    fn construct(&self, _cp: ConstructParams) -> PeerConstructor {
        let r = RandomReader();
        let w = DevNull;
        let p = Peer::new(r, w, None);
        once(Box::new(futures::future::ok(p)) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = RandomClass,
    target = Random,
    prefixes = ["random:"],
    arg_handling = noarg,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Generate random bytes when being read from, discard written bytes.

    websocat -b random: ws://127.0.0.1/flood

"#
);


pub struct RandomReader ();


impl AsyncRead for RandomReader {}

impl Read for RandomReader {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        rand::thread_rng().fill_bytes(buf);
        Ok(buf.len())
    }
}


#[derive(Debug)]
pub struct ExitOnSpecificByte<T: Specifier>(pub T);
impl<T: Specifier> Specifier for ExitOnSpecificByte<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _l2r| {
            Box::new(futures::future::ok(Peer(Box::new(ExitOnSpecificByteReader { 
                inner: p.0,
                the_byte: cp.program_options.byte_to_exit_on,
                eof_triggered: false,
            }), p.1, p.2)))
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = ExitOnSpecificByteClass,
    target = ExitOnSpecificByte,
    prefixes = ["exit_on_specific_byte:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Turn specific byte into a EOF, allowing user to escape interactive Websocat session
when terminal is set to raw mode. Works only bytes read from the overlay, not on the written bytes.

Default byte is 1C which is typically triggered by Ctrl+\.

Example: `(stty raw -echo; websocat -b exit_on_specific_byte:stdio tcp:127.0.0.1:23; stty sane)`

"#
);

pub struct ExitOnSpecificByteReader { 
    inner: Box<dyn AsyncRead>,
    the_byte: u8,
    eof_triggered: bool,
}


impl AsyncRead for ExitOnSpecificByteReader {}

impl Read for ExitOnSpecificByteReader {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if self.eof_triggered {
            return Ok(0);
        }
        let ret = self.inner.read(buf);

        if let Ok(ref sz) = ret {
            let buf = &buf[..*sz];
            if let Some((pos,_)) = buf.iter().enumerate().find(|x|*x.1==self.the_byte) {
                log::info!("Special byte detected. Triggering EOF.");
                self.eof_triggered = true;
                return Ok(pos);
            }   
        }

        ret
    }
}

