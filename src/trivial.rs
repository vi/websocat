use super::{Peer, BoxedNewPeerFuture};

use std;
use futures;
use std::io::{Read,Write};
use std::io::Result as IoResult;

use futures::Async::{Ready};


use tokio_io::{AsyncRead,AsyncWrite};

use super::ReadDebt;
use super::wouldblock;

struct Literal {
    debt: ReadDebt,
}

pub fn get_literal_peer(b:Vec<u8>) -> BoxedNewPeerFuture {
    let r = Literal{debt: ReadDebt(Some(b))};
    let w = DevNull;
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
pub fn get_assert_peer(b:Vec<u8>) -> BoxedNewPeerFuture {
    let r = DevNull;
    let w = Assert(vec![], b);
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}
/// A special peer that returns NotReady without registering for any wakeup, deliberately hanging all connections forever.
pub fn get_constipated_peer() -> BoxedNewPeerFuture {
    let r = Constipated;
    let w = Constipated;
    let p = Peer::new(r,w);
    Box::new(futures::future::ok(p)) as BoxedNewPeerFuture
}


impl AsyncRead for Literal
{}


impl Read for Literal
{
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        Ok(0)
    }
}



struct DevNull;

impl AsyncWrite for DevNull {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
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
impl AsyncRead for DevNull
{}
impl Read for DevNull
{
    fn read(&mut self, _buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        Ok(0)
    }
}


struct Assert(Vec<u8>, Vec<u8>);
impl AsyncWrite for Assert {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        assert_eq!(self.0, self.1);
        info!("Assertion succeed");
        Ok(Ready(()))
    }
}

impl Write for Assert {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

struct Constipated;
impl AsyncWrite for Constipated {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        wouldblock()
    }
}
impl Write for Constipated {
    fn write(&mut self, _buf: &[u8]) -> IoResult<usize> {
        wouldblock()
    }
    fn flush(&mut self) -> IoResult<()> {
        wouldblock()
    }
}
impl AsyncRead for Constipated
{}
impl Read for Constipated
{
    fn read(&mut self, _buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        wouldblock()
    }
}
