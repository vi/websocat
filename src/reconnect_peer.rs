#![allow(unused,dead_code)]

extern crate tokio_io;
extern crate futures;

use futures::future::ok;
use std::rc::Rc;
use std::cell::RefCell;

use super::{Peer, BoxedNewPeerFuture, peer_err};

use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read, Write, Error as IoError};

use std::ops::DerefMut;
use futures::Future;
use super::{once,Specifier,Handle,ProgramState,PeerConstructor};


#[derive(Debug)]
pub struct AutoReconnect<T:Specifier>(pub T);
impl<T:Specifier> Specifier for AutoReconnect<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        if self.0.uses_global_state() {
            let e : Box<::std::error::Error> 
            = "Can't use autoreconnect on a specifier that uses global state".to_owned().into();
            once(Box::new(::futures::future::err(e)) as BoxedNewPeerFuture)
        } else {
            //let inner = self.0.construct(h, ps).get_only_first_conn();
            once(autoreconnector(h.clone(), self.0.clone()))
        }
    }
    specifier_boilerplate!(singleconnect noglobalstate has_subspec typ=Other);
    self_0_is_subspecifier!(...);
    fn clone(&self) -> Box<Specifier> { Box::new(AutoReconnect(self.0.clone())) }
}

struct State {
    s : Box<Specifier>,
    p : RefCell<Option<Peer>>,
}

#[derive(Clone)]
struct PeerHandle(Rc<State>);


impl Read for PeerHandle {
    fn read (&mut self, b:&mut [u8]) -> Result<usize, IoError> {
       unimplemented!()
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write (&mut self, b: &[u8]) -> Result<usize, IoError> {
        unimplemented!()
    }
    fn flush (&mut self) -> Result<(), IoError> {
        unimplemented!()
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(),IoError> {
       unimplemented!()
    }
}


pub fn autoreconnector(h: Handle, s: Box<Specifier>) -> BoxedNewPeerFuture
{
    let s = Rc::new(State{s, p : RefCell::new(None)});
    let ph1 = PeerHandle(s.clone());
    let ph2 = PeerHandle(s);
    let peer = Peer::new(ph1, ph2);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
