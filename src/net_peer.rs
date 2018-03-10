#![allow(unused)]

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use tokio_io::{self,AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use futures::Async::{Ready, NotReady};

use tokio_core::net::{TcpStream, TcpListener, UdpSocket};

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture, peer_err};

/*
struct RcReadProxy<R:AsyncRead>(Rc<R>);

impl<R:AsyncRead> AsyncRead for RcReadProxy<R>{}
impl<R:AsyncRead> Read for RcReadProxy<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        (&*self.0).read(buf)
    }
}

struct RcWriteProxy<W:AsyncWrite>(Rc<W>);

impl<W:AsyncWrite> AsyncWrite for RcWriteProxy<W>{
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        self.0.shutdown()
    }
}
impl<W:AsyncWrite> Write for RcWriteProxy<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}*/

// based on https://github.com/tokio-rs/tokio-core/blob/master/examples/proxy.rs
#[derive(Clone)]
struct MyTcpStream(Rc<TcpStream>);

impl Read for MyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for MyTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsyncRead for MyTcpStream {}

impl AsyncWrite for MyTcpStream {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        try!(self.0.shutdown(std::net::Shutdown::Write));
        Ok(().into())
    }
}

pub fn tcp_connect_peer(handle: &Handle, addr: &str) -> BoxedNewPeerFuture {
    let parsed_addr = match addr.parse() {
        Ok(x) => x,
        Err(e) => return peer_err(e),
    };
    Box::new(
        TcpStream::connect(&parsed_addr, handle).map(|x| {
            let x = Rc::new(x);
            Peer::new(MyTcpStream(x.clone()), MyTcpStream(x.clone()))
        }).map_err(|e|Box::new(e) as Box<std::error::Error>)
    ) as BoxedNewPeerFuture
}

