use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::cell::RefCell;

use std::io::{Error as IoError, Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{once, simple_err, wouldblock};
use futures::{Async, Future, Poll};

#[derive(Debug)]
pub struct Foreachmsg(pub Rc<dyn Specifier>);
impl Specifier for Foreachmsg {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        once(foreachmsg_peer(self.0.clone(), cp))
    }
    specifier_boilerplate!(singleconnect noglobalstate has_subspec);
    self_0_is_subspecifier!(...);
}
specifier_class!(
    name = ForeachmsgClass,
    target = Foreachmsg,
    prefixes = ["foreachmsg:"],
    arg_handling = subspec,
    overlay = true,
    MessageBoundaryStatusDependsOnInnerType,
    SingleConnect,
    help = r#"
Execute something for each incoming message.

Somewhat the reverse of the `autoreconnect:`.

Example:

    websocat -t -u ws://server/listen_for_updates foreachmsg:writefile:status.txt

This keeps only recent incoming message in file and discards earlier messages.
"#
);

#[derive(Default)]
struct State2 {
    already_warned: bool,
}

#[derive(Clone)]
enum Phase {
    Idle,
    WriteDebt(Vec<u8>),
    Flushing,
    Closing,
}

struct State {
    s: Rc<dyn Specifier>,
    p: Option<Peer>,
    n: Option<BoxedNewPeerFuture>,
    cp: ConstructParams,
    aux: State2,
    ph: Phase,
}

/// This implementation's poll is to be reused many times, both after returning item and error
impl State {
    //type Item = &'mut Peer;
    //type Error = Box<::std::error::Error>;

    fn poll(&mut self) -> Poll<&mut Peer, Box<dyn (::std::error::Error)>> {
        let pp = &mut self.p;
        let nn = &mut self.n;

        let aux = &mut self.aux;

        loop {
            let cp = self.cp.clone();
            if let Some(ref mut p) = *pp {
                return Ok(Async::Ready(p));
            }

            // Peer is not present: trying to create a new one

            if let Some(mut bnpf) = nn.take() {
                match bnpf.poll() {
                    Ok(Async::Ready(p)) => {
                        *pp = Some(p);
                        continue;
                    }
                    Ok(Async::NotReady) => {
                        *nn = Some(bnpf);
                        return Ok(Async::NotReady);
                    }
                    Err(_x) => {
                        // Stop on error:
                        //return Err(_x);

                        // Just reconnect again on error

                        if !aux.already_warned {
                            aux.already_warned = true;
                            error!("Reconnecting failed. Trying again in tight endless loop.");
                        }
                    }
                }
            }
            let l2r = cp.left_to_right.clone();
            let pc: PeerConstructor = self.s.construct(cp);
            *nn = Some(pc.get_only_first_conn(l2r));
        }
    }
}

#[derive(Clone)]
struct PeerHandle(Rc<RefCell<State>>);

macro_rules! getpeer {
    ($state:ident -> $p:ident) => {
        let $p: &mut Peer = match $state.poll() {
            Ok(Async::Ready(p)) => p,
            Ok(Async::NotReady) => return wouldblock(),
            Err(e) => {
                return Err(simple_err(format!("{}", e)));
            }
        };
    };
}

impl State {
    fn reconnect(&mut self) {
        info!("Reconnect");
        self.p = None;
        self.ph = Phase::Idle;
    }
}

impl Read for PeerHandle {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        loop {
            let p: &mut Peer = match state.poll() {
                Ok(Async::Ready(p)) => p,
                Ok(Async::NotReady) => return wouldblock(),
                Err(e) => {
                    return Err(simple_err(format!("{}", e)));
                }
            };
            match p.0.read(b) {
                Ok(0) => {
                    return Ok(0);
                }
                Err(e) => {
                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                        return Err(e);
                    }
                    warn!("{}", e);
                    return Err(e);
                }
                Ok(x) => {
                    return Ok(x);
                }
            }
        }
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();

        let mut do_reconnect = false;
        let mut finished = false;
        loop {
            if do_reconnect {
                state.reconnect();
                do_reconnect = false;
            } else if finished {
                state.p = None;
                state.ph = Phase::Idle;
                return Ok(b.len());
            } else {
                let mut ph = state.ph.clone();
                {
                    getpeer!(state -> p);

                    match ph {
                        Phase::Idle => {
                            match p.1.write(b) {
                                Ok(0) => {
                                    info!("End-of-file write?");
                                    return Ok(0);
                                }
                                Err(e) => {
                                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                        return Err(e);
                                    }
                                    warn!("{}", e);
                                    return Err(e);
                                }
                                Ok(x) if x == b.len() => {
                                    debug!("Full write");
                                    // A successful write. Flushing and closing the peer.
                                    ph = Phase::Flushing;
                                }
                                Ok(x) => {
                                    debug!("Partial write of {} bytes", x);
                                    // A partial write. Creating write debt.
                                    let debt = b[x..b.len()].to_vec();
                                    ph = Phase::WriteDebt(debt);
                                }
                            }
                        }
                        Phase::WriteDebt(d) => {
                            match p.1.write(&d[..]) {
                                Ok(0) => {
                                    info!("End-of-file write v2?");
                                    return Ok(0);
                                }
                                Err(e) => {
                                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                        return Err(e);
                                    }
                                    warn!("{}", e);
                                    return Err(e);
                                }
                                Ok(x) if x == d.len() => {
                                    debug!("Closing the debt");
                                    // A successful write. Flushing and closing the peer.
                                    ph = Phase::Flushing;
                                }
                                Ok(x) => {
                                    debug!("Partial write of {} debt bytes", x);
                                    // A partial write. Retaining the write debt.
                                    let debt = d[x..d.len()].to_vec();
                                    ph = Phase::WriteDebt(debt);
                                }
                            }
                        }
                        Phase::Flushing => match p.1.flush() {
                            Err(e) => {
                                if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                    return Err(e);
                                }
                                warn!("{}", e);
                                return Err(e);
                            }
                            Ok(()) => {
                                debug!("Flushed");
                                ph = Phase::Closing;
                            }
                        },
                        Phase::Closing => match p.1.shutdown() {
                            Err(e) => {
                                if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                    return Err(e);
                                }
                                warn!("{}", e);
                                return Err(e);
                            }
                            Ok(Async::NotReady) => {
                                return wouldblock();
                            }
                            Ok(Async::Ready(())) => {
                                debug!("Closed");
                                finished = true;
                            }
                        },
                    }
                }
                state.ph = ph;
            }
        }
    }
    fn flush(&mut self) -> Result<(), IoError> {
        // No-op here: we flush and close after each write
        Ok(())
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), IoError> {
        // No-op here: we flush and close after each write
        Ok(Async::Ready(()))
    }
}

pub fn foreachmsg_peer(s: Rc<dyn Specifier>, cp: ConstructParams) -> BoxedNewPeerFuture {
    let s = Rc::new(RefCell::new(State {
        cp,
        s,
        p: None,
        n: None,
        aux: Default::default(),
        ph: Phase::Idle,
    }));
    let ph1 = PeerHandle(s.clone());
    let ph2 = PeerHandle(s);
    let peer = Peer::new(ph1, ph2, None /* we handle hups ourselves */);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
