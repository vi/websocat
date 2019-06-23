extern crate futures;
extern crate tokio_io;

use futures::future::ok;
use std::cell::RefCell;
use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};

use std::io::{Error as IoError, Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{once, simple_err, wouldblock, ConstructParams, PeerConstructor, Specifier};
use futures::{Async, Future, Poll};

// TODO: shutdown write part if out writing part is shut down
// TODO: stop if writing part and reading parts are closed (shutdown)?

#[derive(Debug)]
pub struct AutoReconnect(pub Rc<dyn Specifier>);
impl Specifier for AutoReconnect {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        once(autoreconnector(self.0.clone(), cp))
    }
    specifier_boilerplate!(singleconnect noglobalstate has_subspec );
    self_0_is_subspecifier!(...);
}
specifier_class!(
    name = AutoReconnectClass,
    target = AutoReconnect,
    prefixes = ["autoreconnect:"],
    arg_handling = subspec,
    overlay = true,
    MessageBoundaryStatusDependsOnInnerType,
    SingleConnect,
    help = r#"
Re-establish underlying connection on any error or EOF

Example: keep connecting to the port or spin 100% CPU trying if it is closed.

    websocat - autoreconnect:tcp:127.0.0.1:5445
    
Example: keep remote logging connection open (or flood the host if port is closed):

    websocat -u ws-l:0.0.0.0:8080 reuse:autoreconnect:tcp:192.168.0.3:1025
  
TODO: implement delays between reconnect attempts
"#
);

#[derive(Default)]
struct State2 {
    already_warned: bool,
}

struct State {
    s: Rc<dyn Specifier>,
    p: Option<Peer>,
    n: Option<BoxedNewPeerFuture>,
    cp: ConstructParams,
    aux: State2,
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
    }
}

macro_rules! main_loop {
    ($state:ident, $p:ident,bytes $e:expr) => {
        main_loop!(qqq $state, $p, do_reconnect, {
                                    match $e {
                                        Ok(0) => { do_reconnect = true; }
                                        Err(e) => {
                                            if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                                return Err(e);
                                            }
                                            warn!("{}", e);
                                            do_reconnect = true;
                                        }
                                        Ok(x) => return Ok(x),
                                    }
                                });
    };
    ($state:ident, $p:ident,none $e:expr) => {
        main_loop!(qqq $state, $p, do_reconnect, {
                                    match $e {
                                        Err(e) => {
                                            if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                                return Err(e);
                                            }
                                            warn!("{}", e);
                                            do_reconnect = true;
                                        }
                                        Ok(()) => return Ok(()),
                                    }
                                });
    };
    (qqq $state:ident, $p:ident, $do_reconnect:ident, $the_match:expr) => {
        let mut $do_reconnect = false;
        loop {
            if $do_reconnect {
                $state.reconnect();
                $do_reconnect = false;
            } else {
                getpeer!($state -> $p);
                $the_match
            }
        }
    };
}

impl Read for PeerHandle {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, bytes p.0.read(b));
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write(&mut self, b: &[u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, bytes p.1.write(b));
    }
    fn flush(&mut self) -> Result<(), IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, none p.1.flush());
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), IoError> {
        let mut state = self.0.borrow_mut();
        state.p = None;
        Ok(Async::Ready(()))
    }
}

pub fn autoreconnector(s: Rc<dyn Specifier>, cp: ConstructParams) -> BoxedNewPeerFuture {
    let s = Rc::new(RefCell::new(State {
        cp,
        s,
        p: None,
        n: None,
        aux: Default::default(),
    }));
    let ph1 = PeerHandle(s.clone());
    let ph2 = PeerHandle(s);
    let peer = Peer::new(ph1, ph2, None /* we handle hups ourselves */);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
