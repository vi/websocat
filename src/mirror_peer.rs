use super::{Peer, BoxedNewPeerFuture};

use super::{io_other_error, brokenpipe, wouldblock};
use std;
use futures;
use futures::sink::Sink;
use futures::stream::Stream;
use std::io::{Read,Write};
use std::io::Result as IoResult;

use futures::Async::{Ready, NotReady};
use std::rc::Rc;

use futures::sync::mpsc;

use tokio_io::{AsyncRead,AsyncWrite};

use super::ReadDebt;
use super::{once,Specifier,ProgramState,Handle,PeerConstructor,Options};

#[derive(Debug,Clone)]
pub struct Mirror;
impl Specifier for Mirror {
    fn construct(&self, _:&Handle, _: &mut ProgramState, _opts: Rc<Options>) -> PeerConstructor {
        once(get_mirror_peer())
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}

#[derive(Clone)]
pub struct LiteralReply(pub Vec<u8>);
impl Specifier for LiteralReply {
    fn construct(&self, _:&Handle, _: &mut ProgramState, _opts: Rc<Options>) -> PeerConstructor {
        once(get_literal_reply_peer(self.0.clone()))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}
impl std::fmt::Debug for LiteralReply{fn fmt(&self, f:&mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> { write!(f, "LiteralReply") }  }






struct MirrorWrite(mpsc::Sender<Vec<u8>>);
struct MirrorRead {
    debt: ReadDebt,
    ch: mpsc::Receiver<Vec<u8>>,
}

pub fn get_mirror_peer() -> BoxedNewPeerFuture {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
    let r = MirrorRead{debt:Default::default(), ch:receiver};
    let w = MirrorWrite(sender);
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
pub fn get_literal_reply_peer(content: Vec<u8>) -> BoxedNewPeerFuture {
    let (sender, receiver) = mpsc::channel::<()>(0);
    let r = LiteralReplyRead{debt:Default::default(), ch:receiver, content};
    let w = LiteralReplyHandle(sender);
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}

impl AsyncRead for MirrorRead
{}


impl Read for MirrorRead
{
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        let r = self.ch.poll();
        match r {
            Ok(Ready(Some(x))) => self.debt.process_message(buf, x.as_slice()),
            Ok(Ready(None)) => brokenpipe(),
            Ok(NotReady) => wouldblock(),
            Err(_) => brokenpipe(),
        }
    }
}


impl AsyncWrite for MirrorWrite {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        Ok(Ready(()))
    }
}

impl Write for  MirrorWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = buf.to_vec();
        match self.0.start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => {
                wouldblock()
            },
            futures::AsyncSink::Ready => {
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.poll_complete().map_err(io_other_error)? {
            NotReady => {
                wouldblock()
            },
            Ready(()) => {
                Ok(())
            }
        }
    }
}

impl Drop for MirrorWrite {
    fn drop(&mut self) {
        info!("MirrorWrite drop");
        let _ = self.0.start_send(vec![])
            .map_err(|_|())
            .map(|_|());
        let _ = self.0.poll_complete()
            .map_err(|_|())
            .map(|_|());
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
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        Ok(Ready(()))
    }
}

impl Write for  LiteralReplyHandle {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = ();
        match self.0.start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => {
                wouldblock()
            },
            futures::AsyncSink::Ready => {
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.poll_complete().map_err(io_other_error)? {
            NotReady => {
                wouldblock()
            },
            Ready(()) => {
                Ok(())
            }
        }
    }
}
impl AsyncRead for LiteralReplyRead
{}
impl Read for LiteralReplyRead
{
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        let r = self.ch.poll();
        match r {
            Ok(Ready(Some(()))) => self.debt.process_message(buf, &self.content),
            Ok(Ready(None)) => brokenpipe(),
            Ok(NotReady) => wouldblock(),
            Err(_) => brokenpipe(),
        }
    }
}

