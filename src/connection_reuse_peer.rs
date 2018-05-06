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
pub struct Reuser<T:Specifier>(pub T);
impl<T:Specifier> Specifier for Reuser<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let mut reuser = ps.reuser.clone();
        let inner = self.0.construct(h, ps).get_only_first_conn();
        once(connection_reuser(&mut reuser, inner))
    }
    specifier_boilerplate!(singleconnect, has_subspec, Reuser);
    self_0_is_subspecifier!(...);
    fn clone(&self) -> Box<Specifier> { Box::new(Reuser(self.0.clone())) }
}


type PeerSlot = Rc<RefCell<Option<Peer>>>;

#[derive(Default,Clone)]
pub struct GlobalState(PeerSlot);

#[derive(Clone)]
struct PeerHandle(PeerSlot);


impl Read for PeerHandle {
    fn read (&mut self, b:&mut [u8]) -> Result<usize, IoError> {
        if let Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.0.read(b)
        } else {
            unreachable!()
        }
    }
}
impl AsyncRead for PeerHandle{}

impl Write for PeerHandle {
    fn write (&mut self, b: &[u8]) -> Result<usize, IoError> {
        if let Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.write(b)
        } else {
            unreachable!()
        }
    }
    fn flush (&mut self) -> Result<(), IoError> {
        if let Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.flush()
        } else {
            unreachable!()
        }
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(),IoError> {
        if let Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.shutdown()
        } else {
            unreachable!()
        }
    }
}


pub fn connection_reuser(s: &mut GlobalState, inner_peer : BoxedNewPeerFuture) -> BoxedNewPeerFuture
{
    let need_init = s.0.borrow().is_none();
    
    let rc = s.0.clone();
    
    if need_init {
        Box::new(inner_peer.and_then(move |inner| {
            {
                let mut b = rc.borrow_mut();
                let x : &mut Option<Peer> = b.deref_mut();
                *x = Some(inner);
            }
            
            let ps : PeerSlot = rc.clone();
        
            let ph1 = PeerHandle(ps);
            let ph2 = ph1.clone();
            let peer = Peer::new(ph1, ph2);
            ok(peer)
        })) as BoxedNewPeerFuture
    } else {
        let ps : PeerSlot = rc.clone();
    
        let ph1 = PeerHandle(ps);
        let ph2 = ph1.clone();
        let peer = Peer::new(ph1, ph2);
        Box::new(ok(peer)) as BoxedNewPeerFuture
    }
}
