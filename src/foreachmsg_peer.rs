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
    WaitingForReadToFinish,
}

struct State {
    s: Rc<dyn Specifier>,
    p: Option<Peer>,
    n: Option<BoxedNewPeerFuture>,
    cp: ConstructParams,
    aux: State2,
    ph: Phase,
    finished_reading: bool,
    read_waiter_tx: Option<futures::sync::oneshot::Sender<()>>,
    read_waiter_rx: Option<futures::sync::oneshot::Receiver<()>>,
    wait_for_new_peer_tx: Option<futures::sync::oneshot::Sender<()>>,
    wait_for_new_peer_rx: Option<futures::sync::oneshot::Receiver<()>>,
    need_wait_for_reading: bool,
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
                        if let Some(tx) = self.wait_for_new_peer_tx.take() {
                            let _ = tx.send(());
                        }
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
            self.finished_reading = false;
            self.ph = Phase::Idle;
            self.read_waiter_tx = None;
            self.read_waiter_rx = None;
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
        self.finished_reading = false;
        self.read_waiter_tx = None;
        self.read_waiter_rx = None;
    }
}

impl Read for PeerHandle {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        loop {
            if let Some(w) = state.wait_for_new_peer_rx.as_mut() {
                match w.poll() {
                    Ok(Async::NotReady) => return wouldblock(),
                    _ => {
                        state.wait_for_new_peer_rx = None;
                    }
                }
            }
            let p : &mut Peer = match state.poll() {
                Ok(Async::Ready(p)) => p,
                Ok(Async::NotReady) => return wouldblock(),
                Err(e) => {
                    return Err(simple_err(format!("{}", e)));
                }
            };
            #[allow(unused_assignments)]
            let mut finished_but_loop_around = false;
            match p.0.read(b) {
                Ok(0) => { 
                    state.finished_reading = true;
                    if state.need_wait_for_reading {
                        finished_but_loop_around = true;
                    } else {
                        return Ok(0);
                    }
                }
                Err(e) => {
                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                        return Err(e);
                    }
                    state.finished_reading = true;
                    warn!("{}", e);

                    if state.need_wait_for_reading {
                        // Get a new peer to read from
                        finished_but_loop_around = true;
                    } else {
                        return Err(e);
                    }
                }
                Ok(x) => {
                    return Ok(x);
                }
            }
            if finished_but_loop_around {
                state.finished_reading = true;
                let (tx,rx) = futures::sync::oneshot::channel();
                state.wait_for_new_peer_tx = Some(tx);
                state.wait_for_new_peer_rx = Some(rx);
                if let Some(rw) = state.read_waiter_tx.take() {
                    let _ = rw.send(());
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
                                },
                                Ok(x) => {
                                    debug!("Partial write of {} bytes", x);
                                    // A partial write. Creating write debt.
                                    let debt = b[x..b.len()].to_vec();
                                    ph = Phase::WriteDebt(debt);
                                }
                            }
                        },
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
                                },
                                Ok(x) => {
                                    debug!("Partial write of {} debt bytes", x);
                                    // A partial write. Retaining the write debt.
                                    let debt = d[x..d.len()].to_vec();
                                    ph = Phase::WriteDebt(debt);
                                }
                            }
                        },
                        Phase::Flushing => {
                            match p.1.flush() {
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
                            }
                        },
                        Phase::Closing => {
                            match p.1.shutdown() {
                                Err(e) => {
                                    if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                        return Err(e);
                                    }
                                    warn!("{}", e);
                                    return Err(e);
                                },
                                Ok(Async::NotReady) => {
                                    return wouldblock();
                                },
                                Ok(Async::Ready(())) => {
                                    if state.need_wait_for_reading {
                                        if state.finished_reading {
                                            debug!("Closed and reading is also done");
                                            finished=true;
                                        } else {
                                            debug!("Closed, but need to wait for other direction to finish");
                                            ph = Phase::WaitingForReadToFinish;
                                            let (tx,rx) = futures::sync::oneshot::channel();
                                            state.read_waiter_tx = Some(tx);
                                            state.read_waiter_rx = Some(rx);
                                        }
                                    } else {
                                        debug!("Closed");
                                        finished=true;
                                    }
                                }
                            }
                        },
                        Phase::WaitingForReadToFinish => {
                            match state.read_waiter_rx.as_mut().unwrap().poll() {
                                Ok(Async::NotReady) => {
                                    return wouldblock();
                                }
                                _ => {
                                    debug!("Waited for read to finish");
                                    finished=true;
                                }
                            }
                        }
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
    let need_wait_for_reading = cp.program_options.foreachmsg_wait_reads;
    let s = Rc::new(RefCell::new(State {
        cp,
        s,
        p: None,
        n: None,
        aux: Default::default(),
        ph: Phase::Idle,
        finished_reading: false,
        read_waiter_tx: None,
        read_waiter_rx: None,
        wait_for_new_peer_rx: None,
        wait_for_new_peer_tx: None,
        need_wait_for_reading,
    }));
    let ph1 = PeerHandle(s.clone());
    let ph2 = PeerHandle(s);
    let peer = Peer::new(ph1, ph2, None /* we handle hups ourselves */);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
