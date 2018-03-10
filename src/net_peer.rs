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

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture};

pub fn tcp_connect_peer(handle: &Handle, addr: &str) -> BoxedNewPeerFuture {
    unimplemented!()
}

