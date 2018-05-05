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
use std::net::SocketAddr;

use std::rc::Rc;
use std::cell::RefCell;

use futures::Async::{Ready, NotReady};

use tokio_core::net::{TcpStream, TcpListener, UdpSocket};

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture, BoxedNewPeerStream, peer_err, peer_err_s, box_up_err};
use super::{once,multi,Specifier,ProgramState,PeerConstructor,StdioUsageStatus};



#[derive(Debug)]
pub struct TcpConnect(pub SocketAddr);
impl Specifier for TcpConnect {
    fn construct(&self, h:&Handle, _: &mut ProgramState) -> PeerConstructor {
        once(tcp_connect_peer(h, &self.0))
    }
    fn is_multiconnect(&self) -> bool { false }
}

#[derive(Debug)]
pub struct TcpListen(pub SocketAddr);
impl Specifier for TcpListen {
    fn construct(&self, h:&Handle, _: &mut ProgramState) -> PeerConstructor {
        multi(tcp_listen_peer(h, &self.0))
    }
    fn is_multiconnect(&self) -> bool { true }
}

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
struct MyTcpStream(Rc<TcpStream>, bool);

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

impl Drop for MyTcpStream {
    fn drop(&mut self) {
        let i_am_read_part = self.1;
        if i_am_read_part {
            let _ = self.0.shutdown(std::net::Shutdown::Read);
        }
    }
}

pub fn tcp_connect_peer(handle: &Handle, addr: &SocketAddr) -> BoxedNewPeerFuture {
    Box::new(
        TcpStream::connect(&addr, handle).map(|x| {
            info!("Connected to TCP");
            let x = Rc::new(x);
            Peer::new(MyTcpStream(x.clone(), true), MyTcpStream(x.clone(), false))
        }).map_err(box_up_err)
    ) as BoxedNewPeerFuture
}

pub fn tcp_listen_peer(handle: &Handle, addr: &SocketAddr) -> BoxedNewPeerStream {
    let bound = match TcpListener::bind(&addr, handle) {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    Box::new(
        bound
        .incoming()
        .map(|(x, _addr)| {
            info!("Incoming TCP connection");
            let x = Rc::new(x);
            Peer::new(MyTcpStream(x.clone(), true), MyTcpStream(x.clone(), false))
        })
        .map_err(|e|box_up_err(e))
    ) as BoxedNewPeerStream
}


