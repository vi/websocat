#![allow(unused)]

extern crate websocket;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_stdin_stdout;

use std::thread;
use std::io::stdin;
use tokio_core::reactor::{Core, Handle};
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use websocket::stream::async::Stream as WsStream;
use futures::Async::{Ready, NotReady};

use tokio_io::io::copy;

use tokio_io::codec::FramedRead;
use std::fs::File;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn wouldblock<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, ""))
}
fn brokenpipe<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))
}
fn io_other_error<E : std::error::Error + Send + Sync + 'static>(e:E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other,e)
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);
type BoxedNewPeerFuture = Box<Future<Item=Peer, Error=Box<std::error::Error>>>;

mod my_copy;

pub mod ws_peer;
pub mod stdio_peer;

impl Peer {
    fn new<R:AsyncRead+'static, W:AsyncWrite+'static>(r:R, w:W) -> Self {
        Peer (
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
    }
}

pub fn peer_from_str(handle: &Handle, s: &str) -> BoxedNewPeerFuture {
    if (s == "-") {
        stdio_peer::get_stdio_peer(handle)
    } else {
        ws_peer::get_ws_client_peer(handle, s)
    }
}

pub struct Transfer {
    from: Box<AsyncRead>,
    to:   Box<AsyncWrite>,
}
pub struct Session(Transfer,Transfer);

type WaitingForImplTraitFeature3 = futures::stream::StreamFuture<futures::sync::mpsc::Receiver<()>>;

impl Session {
    pub fn run(self, handle: &Handle) -> WaitingForImplTraitFeature3 {
        let (notif1,rcv) = mpsc::channel::<()>(0);
        let notif2 = notif1.clone();
        handle.spawn(
            my_copy::copy(self.0.from, self.0.to, true)
                .map_err(|_|())
                .map(|_|{notif1;()})
        );
        handle.spawn(
            my_copy::copy(self.1.from, self.1.to, true)
                .map_err(|_|())
                .map(|_|{notif2;()})
        );
        rcv.into_future()
    }
    pub fn new(peer1: Peer, peer2: Peer) -> Self {
        Session (
            Transfer {
                from: peer1.0,
                to: peer2.1,
            },
            Transfer {
                from: peer2.0,
                to: peer1.1,
            },
        )
    }
}

