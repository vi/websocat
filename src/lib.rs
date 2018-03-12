//! Note: library usage is not semver/API-stable

extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;

use tokio_core::reactor::{Handle};
use futures::future::Future;
use futures::sync::mpsc;
use tokio_io::{AsyncRead,AsyncWrite};

use futures::Stream;


type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn wouldblock<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, ""))
}
fn brokenpipe<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))
}
fn io_other_error<E : std::error::Error + Send + Sync + 'static>(e:E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other,e)
}

#[derive(Default)]
pub struct ProgramState {
    #[cfg(all(unix,not(feature="no_unix_stdio")))]
    stdio : stdio_peer::GlobalState,
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);

pub type BoxedNewPeerFuture = Box<Future<Item=Peer, Error=Box<std::error::Error>>>;
pub type BoxedNewPeerStream = Box<Stream<Item=Peer, Error=Box<std::error::Error>>>;

pub fn fut2str<T:'static,E:'static> (f : Box<Future<Item=T, Error=E>>) -> Box<Stream<Item=T, Error=E>> {
    Box::new(futures::stream::futures_ordered(vec![f])) as Box<Stream<Item=T, Error=E>>
}

pub fn peer_err<E: std::error::Error + 'static>(e : E) -> BoxedNewPeerFuture {
    Box::new(futures::future::err(Box::new(e) as Box<std::error::Error>)) as BoxedNewPeerFuture
}
pub fn peer_strerr(e : &str) -> BoxedNewPeerFuture {
    let q : Box<std::error::Error> = From::from(e);
    Box::new(futures::future::err(q)) as BoxedNewPeerFuture
}
pub fn box_up_err<E: std::error::Error + 'static>(e : E) -> Box<std::error::Error> {
    Box::new(e) as Box<std::error::Error>
}

mod my_copy;

pub mod ws_peer;

pub mod ws_server_peer;
pub mod ws_client_peer;

pub mod net_peer;

#[cfg(all(unix,not(feature="no_unix_stdio")))]
pub mod stdio_peer;

pub mod stdio_threaded_peer;

impl Peer {
    fn new<R:AsyncRead+'static, W:AsyncWrite+'static>(r:R, w:W) -> Self {
        Peer (
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
    }
}

pub fn is_stdio_peer(s: &str) -> bool {
    match s {
        "-" => true,
        "inetd:" => true,
        "threadedstdio:" => true,
        _ => false,
    }
}

pub fn is_stdioish_peer(s: &str) -> bool {
    if is_stdio_peer(s) {
        true
    } else {
        if let Some(x) = ws_l_prefix(s) {
            is_stdioish_peer(x)
        } else
        if let Some(x) = ws_c_prefix(s) {
            is_stdioish_peer(x)
        } else {
            false
        }
    }
}

pub fn ws_l_prefix(s:&str) -> Option<&str> {
    if    s.starts_with("ws-l:") 
       || s.starts_with("l-ws:")
    {
        Some(&s[5..])
    }
    else if  s.starts_with("ws-listen:")
          || s.starts_with("listen-ws:")
    {
        Some(&s[10..])
    } else {
        None
    }
}

pub fn ws_c_prefix(s:&str) -> Option<&str> {
    if    s.starts_with("ws-c:") 
       || s.starts_with("c-ws:")
    {
        Some(&s[5..])
    }
    else if  s.starts_with("ws-connect:")
          || s.starts_with("connect-ws:")
    {
        Some(&s[11..])
    } else {
        None
    }
}


pub fn peer_from_str(ps: &mut ProgramState, handle: &Handle, s: &str) -> BoxedNewPeerStream {
    if s == "-" || s == "inetd:" {
        let ret;
        #[cfg(all(unix,not(feature="no_unix_stdio")))]
        {
            ret = stdio_peer::get_stdio_peer(&mut ps.stdio, handle)
        }
        #[cfg(any(not(unix),feature="no_unix_stdio"))]
        {
            ret = stdio_threaded_peer::get_stdio_peer()
        }
        fut2str(ret)
    } else 
    if s == "threadedstdio:" {
        fut2str(stdio_threaded_peer::get_stdio_peer())
    } else 
    if s.starts_with("tcp:") {
        fut2str(net_peer::tcp_connect_peer(handle, &s[4..]))
    } else 
    if s.starts_with("tcp-connect:") {
        fut2str(net_peer::tcp_connect_peer(handle, &s[12..]))
    } else 
    if s.starts_with("tcp-l:") {
        fut2str(net_peer::tcp_listen_peer(handle, &s[6..]))
    } else 
    if s.starts_with("l-tcp:") {
        fut2str(net_peer::tcp_listen_peer(handle, &s[6..]))
    } else 
    if s.starts_with("tcp-listen:") {
        fut2str(net_peer::tcp_listen_peer(handle, &s[11..]))
    } else 
    if let Some(x) = ws_l_prefix(s) {
        if x == "" {
            return fut2str(peer_strerr("Specify underlying protocol for ws-l:"))
        }
        if let Some(c) = x.chars().next() {
            if c.is_numeric() || c == '[' {
                // Assuming user uses old format like ws-l:127.0.0.1:8080
                return peer_from_str(ps, handle, &("ws-l:tcp-l:".to_owned() + x));
            }
        }
        let inner = peer_from_str(ps, handle, x);
        Box::new(inner.and_then(ws_server_peer::ws_upgrade_peer)) as BoxedNewPeerStream
    } else 
    if let Some(x) = ws_c_prefix(s) {
        let inner = peer_from_str(ps, handle, x);
        Box::new(inner.and_then(|q| {
            ws_client_peer::get_ws_client_peer_wrapped("ws://0.0.0.0/", q)
        })) as BoxedNewPeerStream
    } else 
    {
        fut2str(ws_client_peer::get_ws_client_peer(handle, s))
    }
}

pub struct Transfer {
    from: Box<AsyncRead>,
    to:   Box<AsyncWrite>,
}
pub struct Session(Transfer,Transfer);

type WaitingForImplTraitFeature3 = futures::stream::StreamFuture<futures::sync::mpsc::Receiver<()>>;

impl Session {
    pub fn run(self, handle: &Handle) -> WaitingForImplTraitFeature3 {
        let (notif1,rcv) = mpsc::channel::<()>(0);
        let notif2 = notif1.clone();
        handle.spawn(
            my_copy::copy(self.0.from, self.0.to, true)
                .map_err(|_|())
                .map(move |_|{
                    std::mem::drop(notif1);
                    ()
                })
        );
        handle.spawn(
            my_copy::copy(self.1.from, self.1.to, true)
                .map_err(|_|())
                .map(move |_|{
                    std::mem::drop(notif2);
                    ()
                })
        );
        rcv.into_future()
    }
    pub fn new(peer1: Peer, peer2: Peer) -> Self {
        Session (
            Transfer {
                from: peer1.0,
                to: peer2.1,
            },
            Transfer {
                from: peer2.0,
                to: peer1.1,
            },
        )
    }
}

