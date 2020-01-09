use super::{BoxedNewPeerFuture, Peer};

use super::{brokenpipe, io_other_error, wouldblock};
use futures;
use futures::sink::Sink;
use futures::stream::Stream;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};

use futures::Async::{NotReady, Ready};
use std::rc::Rc;

use futures::sync::mpsc;

use tokio_io::{AsyncRead, AsyncWrite};

use super::readdebt::{DebtHandling, ProcessMessageResult, ReadDebt, ZeroMessagesHandling};
use super::{once, ConstructParams, PeerConstructor, Specifier};

#[derive(Debug, Clone)]
pub struct Mirror;
impl Specifier for Mirror {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        once(get_mirror_peer(cp.program_options.read_debt_handling))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = MirrorClass,
    target = Mirror,
    prefixes = ["mirror:"],
    arg_handling = noarg,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Simply copy output to input. No arguments needed.

Example: emulate echo.websocket.org

    websocat -t ws-l:127.0.0.1:1234 mirror:
"#
);
// TODO: doc example, mention echo.websocket.org

#[derive(Clone)]
pub struct LiteralReply(pub Vec<u8>);
impl Specifier for LiteralReply {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(get_literal_reply_peer(self.0.clone()))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
impl std::fmt::Debug for LiteralReply {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "LiteralReply")
    }
}
specifier_class!(
    name = LiteralReplyClass,
    target = LiteralReply,
    prefixes = ["literalreply:"],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Reply with a specified string for each input packet.

Example:

    websocat ws-l:0.0.0.0:1234 literalreply:'{"status":"OK"}'
"#
);

struct MirrorWrite(mpsc::Sender<Vec<u8>>);
struct MirrorRead {
    debt: ReadDebt,
    ch: mpsc::Receiver<Vec<u8>>,
}

pub fn get_mirror_peer(debt_handling: DebtHandling) -> BoxedNewPeerFuture {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
    let r = MirrorRead {
        debt: ReadDebt(Default::default(), debt_handling, ZeroMessagesHandling::Deliver),
        ch: receiver,
    };
    let w = MirrorWrite(sender);
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
pub fn get_literal_reply_peer(content: Vec<u8>) -> BoxedNewPeerFuture {
    let (sender, receiver) = mpsc::channel::<()>(0);
    let r = LiteralReplyRead {
        debt: ReadDebt(Default::default(), DebtHandling::Silent, ZeroMessagesHandling::Deliver),
        ch: receiver,
        content,
    };
    let w = LiteralReplyHandle(sender);
    let p = Peer::new(r, w, None);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}

impl AsyncRead for MirrorRead {}

impl Read for MirrorRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        loop {
            let r = self.ch.poll();
            return match r {
                Ok(Ready(Some(x))) => match self.debt.process_message(buf, x.as_slice()) {
                    ProcessMessageResult::Return(x) => x,
                    ProcessMessageResult::Recurse => continue,
                },
                Ok(Ready(None)) => brokenpipe(),
                Ok(NotReady) => wouldblock(),
                Err(_) => brokenpipe(),
            };
        }
    }
}

impl AsyncWrite for MirrorWrite {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(Ready(()))
    }
}

impl Write for MirrorWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = buf.to_vec();
        match self.0.start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => wouldblock(),
            futures::AsyncSink::Ready => Ok(buf.len()),
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.poll_complete().map_err(io_other_error)? {
            NotReady => wouldblock(),
            Ready(()) => Ok(()),
        }
    }
}

impl Drop for MirrorWrite {
    fn drop(&mut self) {
        info!("MirrorWrite drop");
        let _ = self.0.start_send(vec![]).map_err(|_| ()).map(|_| ());
        let _ = self.0.poll_complete().map_err(|_| ()).map(|_| ());
    }
}

////
struct LiteralReplyHandle(mpsc::Sender<()>);
struct LiteralReplyRead {
    debt: ReadDebt,
    ch: mpsc::Receiver<()>,
    content: Vec<u8>,
}

impl AsyncWrite for LiteralReplyHandle {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(Ready(()))
    }
}

impl Write for LiteralReplyHandle {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self.0.start_send(()).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => wouldblock(),
            futures::AsyncSink::Ready => Ok(buf.len()),
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.poll_complete().map_err(io_other_error)? {
            NotReady => wouldblock(),
            Ready(()) => Ok(()),
        }
    }
}
impl AsyncRead for LiteralReplyRead {}
impl Read for LiteralReplyRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        loop {
            let r = self.ch.poll();
            return match r {
                Ok(Ready(Some(()))) => match self.debt.process_message(buf, &self.content) {
                    ProcessMessageResult::Return(x) => x,
                    ProcessMessageResult::Recurse => continue,
                },
                Ok(Ready(None)) => brokenpipe(),
                Ok(NotReady) => wouldblock(),
                Err(_) => brokenpipe(),
            };
        }
    }
}
