extern crate tokio_io;
extern crate futures;

use futures::future::ok;
use std::rc::Rc;
use std::cell::RefCell;

use super::{Peer, BoxedNewPeerFuture};

use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read, Write, Error as IoError};

use futures::{Future,Poll,Async};
use super::{once,Specifier,Handle,ProgramState,PeerConstructor,wouldblock,Options};

// TODO: shutdown write part if out writing part is shut down
// TODO: stop if writing part and reading parts are closed (shutdown)?


#[derive(Debug)]
pub struct AutoReconnect(pub Rc<Specifier>);
impl Specifier for AutoReconnect {
    fn construct(&self, h:&Handle, _ps: &mut ProgramState, opts: &Options) -> PeerConstructor {
        let mut subspec_globalstate = false;
        let opts = opts.clone();
        
        for i in self.0.get_info().collect() {
            if i.uses_global_state { 
                subspec_globalstate = true;
            }
        }
        
        if subspec_globalstate {
            let e : Box<::std::error::Error> 
            = "Can't use autoreconnect on a specifier that uses global state".to_owned().into();
            once(Box::new(::futures::future::err(e)) as BoxedNewPeerFuture)
        } else {
            //let inner = self.0.construct(h, ps).get_only_first_conn();
            once(autoreconnector(h.clone(), self.0.clone(), opts))
        }
    }
    specifier_boilerplate!(singleconnect noglobalstate has_subspec typ=Other);
    self_0_is_subspecifier!(...);
}

#[derive(Default)]
struct State2 {
    already_warned : bool,
}

struct State {
    s : Rc<Specifier>,
    p : Option<Peer>,
    n : Option<BoxedNewPeerFuture>,
    h : Handle,
    opts: Options,
    aux : State2,
}

/// This implementation's poll is to be reused many times, both after returning item and error
impl /*Future for */ State {
    //type Item = &'mut Peer;
    //type Error = Box<::std::error::Error>;
    
    fn poll(&mut self) -> Poll<&mut Peer, Box<::std::error::Error>> {
        let pp = &mut self.p;
        let nn = &mut self.n;
        
        let aux = &mut self.aux;
        let opts = &self.opts;
        
        loop {
            if let Some(ref mut p) = pp {
                return Ok(Async::Ready(p));
            }
            
            // Peer is not present: trying to create a new one
            
            if let Some(ref mut bnpf) = nn {
                match bnpf.poll() {
                    Ok(Async::Ready(p)) => {
                        *pp = Some(p);
                        *nn = None;
                        continue;
                    },
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(_x) => {
                        // Stop on error:
                        //return Err(_x);
                        
                        // Just reconnect again on error:
                        *nn = None;
                        
                        if ! aux.already_warned {
                            aux.already_warned = true;
                            error!("Reconnecting failed. Trying again in tight endless loop.");
                        }
                    },
                }
            }
            
            let mut fake_ps : ProgramState = Default::default();
            let pc : PeerConstructor = self.s.construct(&self.h, &mut fake_ps, opts);
            *nn = Some(pc.get_only_first_conn());
        }
    }
}

#[derive(Clone)]
struct PeerHandle(Rc<RefCell<State>>);

macro_rules! getpeer {
    ($state:ident -> $p:ident) => {
        let $p : &mut Peer = 
        match $state.poll() {
            Ok(Async::Ready(p)) => p,
            Ok(Async::NotReady) => return wouldblock(),
            Err(e) => {
                let e1 : Box<::std::error::Error+Send+Sync+'static> = format!("{}",e).into();
                let e2 = ::std::io::Error::new(::std::io::ErrorKind::Other,e1);
                return Err(e2);
            },
        };
    }
}


impl State {
    fn reconnect(&mut self) {
        info!("Reconnect");
        self.p = None;
    }
}

macro_rules! main_loop {
    ($state:ident, $p:ident, bytes $e:expr) => {
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
    ($state:ident, $p:ident, none $e:expr) => {
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
    }
}

impl Read for PeerHandle {
    fn read (&mut self, b:&mut [u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, bytes p.0.read(b));
    }
}
impl AsyncRead for PeerHandle {}

impl Write for PeerHandle {
    fn write (&mut self, b: &[u8]) -> Result<usize, IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, bytes p.1.write(b));
    }
    fn flush (&mut self) -> Result<(), IoError> {
        let mut state = self.0.borrow_mut();
        main_loop!(state, p, none p.1.flush());
    }
}
impl AsyncWrite for PeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(),IoError> {
       let mut state = self.0.borrow_mut();
       state.p = None;
       Ok(Async::Ready(()))
    }
}


pub fn autoreconnector(h: Handle, s: Rc<Specifier>, opts: Options) -> BoxedNewPeerFuture
{
    let s = Rc::new(RefCell::new(
        State{
            h,
            s, 
            p : None, 
            n: None,
            aux: Default::default(),
            opts,
    }));
    let ph1 = PeerHandle(s.clone());
    let ph2 = PeerHandle(s);
    let peer = Peer::new(ph1, ph2);
    Box::new(ok(peer)) as BoxedNewPeerFuture
}
