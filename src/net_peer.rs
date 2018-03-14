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

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture, BoxedNewPeerStream, peer_err, peer_err_s, box_up_err};

/*
struct RcReadProxy<R>(Rc<R>) where for<'a> &'a R : AsyncRead;

impl<R> AsyncRead for RcReadProxy<R> where for<'a> &'a R : AsyncRead{}
impl<R> Read for RcReadProxy<R> where for<'a> &'a R : AsyncRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        (&*self.0).read(buf)
    }
}

struct RcWriteProxy<W>(Rc<W>) where for<'a> &'a W : AsyncWrite;

impl<W> AsyncWrite for RcWriteProxy<W> where for<'a> &'a W : AsyncWrite {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        (&*self.0).shutdown()
    }
}
impl<W> Write for RcWriteProxy<W> where for<'a> &'a W : AsyncWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        (&*self.0).flush()
    }
}
*/

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
        }).map_err(box_up_err)
    ) as BoxedNewPeerFuture
}

pub fn tcp_listen_peer(handle: &Handle, addr: &str) -> BoxedNewPeerStream {
    let parsed_addr = match addr.parse() {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    let bound = match TcpListener::bind(&parsed_addr, handle) {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    Box::new(
        bound
        .incoming()
        .map(|(x, _addr)| {
            let x = Rc::new(x);
            Peer::new(MyTcpStream(x.clone()), MyTcpStream(x.clone()))
        })
        .map_err(|e|box_up_err(e))
    ) as BoxedNewPeerStream
}


