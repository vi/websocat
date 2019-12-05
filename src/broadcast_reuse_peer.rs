extern crate futures;
extern crate tokio_io;

use futures::future::ok;
use std::cell::RefCell;
use std::rc::Rc;

use super::{brokenpipe, simple_err, wouldblock, BoxedNewPeerFuture, Peer};

use std::io::{Error as IoError, Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{once, ConstructParams, PeerConstructor, Specifier};
use futures::Async;
use futures::AsyncSink;
use futures::Future;
use futures::Sink;
use futures::Stream;
use crate::spawn_hack;
use std::ops::DerefMut;

use futures::unsync::mpsc;

declare_slab_token!(BroadcastClientIndex);
use slab_typesafe::Slab;

#[derive(Debug)]
pub struct BroadcastReuser(pub Rc<dyn Specifier>);
impl Specifier for BroadcastReuser {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let mut reuser = p.global(GlobalState::default).clone();
        let bs = p.program_options.buffer_size;
        let ql = p.program_options.broadcast_queue_len;
        let l2r = p.left_to_right.clone();
        let inner = || self.0.construct(p).get_only_first_conn(l2r);
        once(connection_reuser(&mut reuser, inner, bs, ql))
    }
    specifier_boilerplate!(singleconnect has_subspec globalstate);
    self_0_is_subspecifier!(...);
}

specifier_class!(
    name = BroadcastReuserClass,
    target = BroadcastReuser,
    prefixes = [
        "broadcast:",
        "reuse:",
        "reuse-broadcast:",
        "broadcast-reuse:"
    ],
    arg_handling = subspec,
    overlay = true,
    MessageBoundaryStatusDependsOnInnerType,
    SingleConnect,
    help = r#"
Reuse this connection for serving multiple clients, sending replies to all clients.

Messages from any connected client get directed to inner connection,
replies from the inner connection get duplicated across all connected
clients (and are dropped if there are none).

If WebSocket client is too slow for accepting incoming data,
messages get accumulated up to the configurable --broadcast-buffer, then dropped.

Example: Simple data exchange between connected WebSocket clients

    websocat -E ws-l:0.0.0.0:8800 reuse-broadcast:mirror:
"#
);

type SailingBuffer = Rc<Vec<u8>>;
type Clients = Slab<BroadcastClientIndex, mpsc::Sender<SailingBuffer>>;

pub struct Broadcaster {
    inner_peer: Peer,
    clients: Clients,
}
pub type HBroadCaster = Rc<RefCell<Option<Broadcaster>>>;

pub type GlobalState = HBroadCaster;

struct PeerHandleW(HBroadCaster);
struct PeerHandleR(
    HBroadCaster,
    mpsc::Receiver<SailingBuffer>,
    BroadcastClientIndex,
);
struct InnerPeerReader(HBroadCaster, Vec<u8>);

impl Future for InnerPeerReader {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> futures::Poll<(), ()> {
        loop {
            let mut meb = self.0.borrow_mut();
            let me = meb.as_mut().expect("Assertion failed 16293");
            match me.inner_peer.0.read(&mut self.1[..]) {
                Ok(0) => {
                    info!("Underlying peer finished");
                    return Ok(futures::Async::Ready(()));
                }
                Ok(n) => {
                    if me.clients.is_empty() {
                        info!("Dropping broadcast due to no clients being connected");
                        continue;
                    };
                    let sb = Rc::new(self.1[0..n].to_vec());
                    for (_, client) in me.clients.iter_mut() {
                        match client.start_send(sb.clone()) {
                            Ok(AsyncSink::Ready) => match client.poll_complete() {
                                Ok(Async::Ready(())) => {}
                                Ok(Async::NotReady) => {
                                    warn!("A client's sink is NotReady for poll_complete");
                                }
                                Err(e) => {
                                    warn!("A client's sink is in error state: {}", e);
                                }
                            },
                            Ok(AsyncSink::NotReady(_)) => {
                                warn!("A client's sink is NotReady for start_send");
                            }
                            Err(e) => {
                                warn!("A client's sink is in error state: {}", e);
                            }
                        };
                    }
                }
                Err(e) => {
                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                        return Ok(Async::NotReady);
                    }
                    error!("Inner peer read failed: {}", e);
                    return Err(());
                }
            }
        }
    }
}

impl Drop for PeerHandleR {
    fn drop(&mut self) {
        self.0
            .borrow_mut()
            .as_mut()
            .expect("Assertion failed 16292")
            .clients
            .remove(self.2);
    }
}

impl Read for PeerHandleR {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        loop {
            return match self.1.poll() {
                Ok(Async::Ready(Some(v))) => {
                    if v.len() > b.len() {
                        error!("Too big message dropped");
                        continue;
                    }
                    b[0..(v.len())].copy_from_slice(&v[..]);
                    Ok(v.len())
                }
                Ok(Async::Ready(None)) => brokenpipe(),
                Ok(Async::NotReady) => wouldblock(),
                Err(()) => Err(simple_err("Something unexpected".into())),
            };
        }

        /*if let &mut Some(ref mut x) = self.0.borrow_mut().deref_mut() {
            x.inner_peer.0.read(b) // To be changed
        } else {
            unreachable!()
        }*/
    }
}
impl AsyncRead for PeerHandleR {}

impl Write for PeerHandleW {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        if let Some(ref mut x) = *self.0.borrow_mut().deref_mut() {
            x.inner_peer.1.write(b)
        } else {
            unreachable!()
        }
    }
    fn flush(&mut self) -> Result<(), IoError> {
        if let Some(ref mut x) = *self.0.borrow_mut().deref_mut() {
            x.inner_peer.1.flush()
        } else {
            unreachable!()
        }
    }
}
impl AsyncWrite for PeerHandleW {
    fn shutdown(&mut self) -> futures::Poll<(), IoError> {
        if let Some(ref mut _x) = *self.0.borrow_mut().deref_mut() {
            // Ignore shutdown attempts
            Ok(futures::Async::Ready(()))
        //_x.1.shutdown()
        } else {
            unreachable!()
        }
    }
}

fn makeclient(ps: HBroadCaster, queue_len: usize) -> Peer {
    let (send, recv) = mpsc::channel(queue_len);
    let k = ps
        .borrow_mut()
        .as_mut()
        .expect("Assertion failed 16291")
        .clients
        .insert(send);
    let ph1 = PeerHandleR(ps.clone(), recv, k);
    let ph2 = PeerHandleW(ps);
    Peer::new(ph1, ph2, None /* TODO */)
}

pub fn connection_reuser<F: FnOnce() -> BoxedNewPeerFuture>(
    s: &mut GlobalState,
    inner_peer: F,
    buffer_size: usize,
    queue_len: usize,
) -> BoxedNewPeerFuture {
    let need_init = s.borrow().is_none();

    let rc = s.clone();
    if need_init {
        info!("Initializing");
        Box::new(inner_peer().and_then(move |inner| {
            {
                let mut b = rc.borrow_mut();
                let x: &mut Option<Broadcaster> = b.deref_mut();
                *x = Some(Broadcaster {
                    inner_peer: inner,
                    clients: Clients::new(),
                });
                spawn_hack(InnerPeerReader(rc.clone(), vec![0; buffer_size]));
            }

            let ps: HBroadCaster = rc.clone();
            ok(makeclient(ps, queue_len))
        })) as BoxedNewPeerFuture
    } else {
        info!("Reusing");
        let ps: HBroadCaster = rc.clone();
        Box::new(ok(makeclient(ps, queue_len))) as BoxedNewPeerFuture
    }
}
