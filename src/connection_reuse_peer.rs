extern crate futures;
extern crate tokio_io;

use futures::future::ok;
use std::cell::RefCell;
use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};

use std::io::{Error as IoError, Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{once, Handle, Options, PeerConstructor, ProgramState, Specifier};
use futures::Future;
use std::ops::DerefMut;

#[derive(Debug)]
pub struct Reuser(pub Rc<Specifier>);
impl Specifier for Reuser {
    fn construct(&self, h: &Handle, ps: &mut ProgramState, opts: Rc<Options>) -> PeerConstructor {
        let mut reuser = ps.reuser.clone();
        let inner = || self.0.construct(h, ps, opts).get_only_first_conn();
        once(connection_reuser(&mut reuser, inner))
    }
    specifier_boilerplate!(singleconnect has_subspec typ=Reuser globalstate);
    self_0_is_subspecifier!(...);
}

type PeerSlot = Rc<RefCell<Option<Peer>>>;

#[derive(Default, Clone)]
pub struct GlobalState(PeerSlot);

#[derive(Clone)]
struct PeerHandle(PeerSlot);

impl Read for PeerHandle {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        if let &mut Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.0.read(b)
        } else {
            unreachable!()
        }
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        if let &mut Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.write(b)
        } else {
            unreachable!()
        }
    }
    fn flush(&mut self) -> Result<(), IoError> {
        if let &mut Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.flush()
        } else {
            unreachable!()
        }
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), IoError> {
        if let &mut Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.1.shutdown()
        } else {
            unreachable!()
        }
    }
}

pub fn connection_reuser<F: FnOnce() -> BoxedNewPeerFuture>(
    s: &mut GlobalState,
    inner_peer: F,
) -> BoxedNewPeerFuture {
    let need_init = s.0.borrow().is_none();

    let rc = s.0.clone();

    if need_init {
        info!("Initializing");
        Box::new(inner_peer().and_then(move |inner| {
            {
                let mut b = rc.borrow_mut();
                let x: &mut Option<Peer> = b.deref_mut();
                *x = Some(inner);
            }

            let ps: PeerSlot = rc.clone();

            let ph1 = PeerHandle(ps);
            let ph2 = ph1.clone();
            let peer = Peer::new(ph1, ph2);
            ok(peer)
        })) as BoxedNewPeerFuture
    } else {
        info!("Reusing");
        let ps: PeerSlot = rc.clone();

        let ph1 = PeerHandle(ps);
        let ph2 = ph1.clone();
        let peer = Peer::new(ph1, ph2);
        Box::new(ok(peer)) as BoxedNewPeerFuture
    }
}
