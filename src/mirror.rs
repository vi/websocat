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

struct MirrorWrite(mpsc::Sender<Vec<u8>>);
struct MirrorRead {
    debt: Option<Vec<u8>>,
    ch: mpsc::Receiver<Vec<u8>>,
}

pub fn get_mirror_peer() -> BoxedNewPeerFuture {
    let (sender, receiver) = mpsc::channel::<Vec<u8>>(0);
    let r = MirrorRead{debt:None, ch:receiver};
    let w = MirrorWrite(sender);
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}


impl MirrorRead {
    fn process_message(&mut self, buf: &mut [u8], buf_in: &[u8]) -> std::result::Result<usize, std::io::Error> {
        let l = buf_in.len().min(buf.len());
        buf[..l].copy_from_slice(&buf_in[..l]);
        
        if l < buf_in.len() {
            self.debt = Some(buf_in[l..].to_vec());
        }
        
        Ok(l)
    }
}


impl AsyncRead for MirrorRead
{}


impl Read for MirrorRead
{
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(debt) = self.debt.take() {
            return self.process_message(buf, debt.as_slice());
        }
        let r = self.ch.poll();
        match r {
            Ok(Ready(Some(x))) => self.process_message(buf, x.as_slice()),
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
        let _ = self.0.start_send(vec![])
            .map_err(|_|())
            .map(|_|());
        let _ = self.0.poll_complete()
            .map_err(|_|())
            .map(|_|());
    }
}
