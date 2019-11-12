extern crate futures;
extern crate tokio_io;

use futures::future::ok;
use std::cell::RefCell;
use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};

use std::io::{Error as IoError, Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{once, ConstructParams, PeerConstructor, Specifier};
use futures::Future;
use std::ops::DerefMut;

#[derive(Debug)]
pub struct Reuser(pub Rc<dyn Specifier>);
impl Specifier for Reuser {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let send_zero_msg_on_disconnect = p.program_options.reuser_send_zero_msg_on_disconnect;
        let reuser = p.global(GlobalState::default).clone();
        let mut reuser = reuser.clone();
        let l2r = p.left_to_right.clone();
        let inner = || self.0.construct(p).get_only_first_conn(l2r);
        once(connection_reuser(
            &mut reuser,
            inner,
            send_zero_msg_on_disconnect,
        ))
    }
    specifier_boilerplate!(singleconnect has_subspec globalstate);
    self_0_is_subspecifier!(...);
}

specifier_class!(
    name = ReuserClass,
    target = Reuser,
    prefixes = ["reuse-raw:", "raw-reuse:"],
    arg_handling = subspec,
    overlay = true,
    MessageBoundaryStatusDependsOnInnerType,
    SingleConnect,
    help = r#"
Reuse subspecifier for serving multiple clients: unpredictable mode. [A]

Better used with --unidirectional, otherwise replies get directed to
random connected client.

Example: Forward multiple parallel WebSocket connections to a single persistent TCP connection

    websocat -u ws-l:0.0.0.0:8800 reuse:tcp:127.0.0.1:4567

Example (unreliable): don't disconnect SSH when websocket reconnects

    websocat ws-l:[::]:8088 reuse:tcp:127.0.0.1:22
"#
);

type PeerSlot = Rc<RefCell<Option<Peer>>>;

#[derive(Default, Clone)]
pub struct GlobalState(PeerSlot);

#[derive(Clone)]
struct PeerHandle(PeerSlot, bool);

impl Read for PeerHandle {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        if let Some(ref mut x) = *self.0.borrow_mut().deref_mut() {
            x.0.read(b)
        } else {
            unreachable!()
        }
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        if let Some(ref mut x) = *self.0.borrow_mut().deref_mut() {
            x.1.write(b)
        } else {
            unreachable!()
        }
    }
    fn flush(&mut self) -> Result<(), IoError> {
        if let Some(ref mut x) = *self.0.borrow_mut().deref_mut() {
            x.1.flush()
        } else {
            unreachable!()
        }
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), IoError> {
        if self.1 {
            let _ = self.write(b"");
        }
        if let Some(ref mut _x) = *self.0.borrow_mut().deref_mut() {
            // Ignore shutdown attempts
            Ok(futures::Async::Ready(()))
        //_x.1.shutdown()
        } else {
            unreachable!()
        }
    }
}

pub fn connection_reuser<F: FnOnce() -> BoxedNewPeerFuture>(
    s: &mut GlobalState,
    inner_peer: F,
    send_zero_msg_on_disconnect: bool,
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

            let ph1 = PeerHandle(ps, send_zero_msg_on_disconnect);
            let ph2 = ph1.clone();
            let peer = Peer::new(ph1, ph2, None /* TODO */);
            ok(peer)
        })) as BoxedNewPeerFuture
    } else {
        info!("Reusing");
        let ps: PeerSlot = rc.clone();

        let ph1 = PeerHandle(ps, send_zero_msg_on_disconnect);
        let ph2 = ph1.clone();
        let peer = Peer::new(ph1, ph2, None /* TODO */);
        Box::new(ok(peer)) as BoxedNewPeerFuture
    }
}
