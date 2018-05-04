use super::{Peer, BoxedNewPeerFuture};

use super::{io_other_error, brokenpipe, wouldblock};
use std;
use futures;
use futures::sink::Sink;
use futures::stream::Stream;
use std::io::{Read,Write};
use std::io::Result as IoResult;

use futures::Async::{Ready, NotReady};


use futures::sync::mpsc;

use tokio_io::{AsyncRead,AsyncWrite};

use super::ReadDebt;

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
