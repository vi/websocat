#![allow(unused,dead_code)]

extern crate tokio_io;
extern crate futures;

use futures::future::ok;
use std::rc::Rc;
use std::cell::RefCell;

use super::{Peer, BoxedNewPeerFuture};

use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read, Write, Error as IoError};

use std::ops::DerefMut;
use futures::Future;
use super::{once,Specifier,Handle,ProgramState,PeerConstructor};


#[derive(Debug)]
pub struct AutoReconnect<T:Specifier>(pub T);
impl<T:Specifier> Specifier for AutoReconnect<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        //let inner = self.0.construct(h, ps).get_only_first_conn();
        once(autoreconnector(self.0.clone()))
    }
    specifier_boilerplate!(singleconnect, has_subspec, Reuser);
    self_0_is_subspecifier!(...);
    fn clone(&self) -> Box<Specifier> { Box::new(AutoReconnect(self.0.clone())) }
}

#[derive(Clone)]
struct PeerHandle<S:Specifier>(S);


impl<S:Specifier> Read for PeerHandle<S> {
    fn read (&mut self, b:&mut [u8]) -> Result<usize, IoError> {
       unimplemented!()
    }
}
impl<S:Specifier> AsyncRead for PeerHandle<S>{}

impl<S:Specifier> Write for PeerHandle<S> {
    fn write (&mut self, b: &[u8]) -> Result<usize, IoError> {
        unimplemented!()
    }
    fn flush (&mut self) -> Result<(), IoError> {
        unimplemented!()
    }
}
impl<S:Specifier> AsyncWrite for PeerHandle<S> {
    fn shutdown(&mut self) -> futures::Poll<(),IoError> {
       unimplemented!()
    }
}


pub fn autoreconnector<S:Specifier>(s: S) -> BoxedNewPeerFuture
{
    let ph1 = unimplemented!() as Box<AsyncRead>;
    let ph2 = unimplemented!() as Box<AsyncWrite>;
    let peer = Peer::new(ph1, ph2);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
