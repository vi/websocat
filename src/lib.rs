//! Note: library usage is not semver/API-stable
//!
//! Abstract type evolution of an endpoint:
//! 1. `&str` - string as passed to command line
//! 2. `Specifier` - more organized representation, maybe nested
//! 3. `PeerConstructor` - a future or stream that returns one or more connections
//! 4. `Peer` - one active connection

extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate websocket;

use tokio_core::reactor::{Handle};
use futures::future::Future;
use tokio_io::{AsyncRead,AsyncWrite};

use futures::Stream;

use websocket::client::Url;
use std::net::SocketAddr;

use std::str::FromStr;


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
    
    reuser: connection_reuse_peer::GlobalState,
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);

pub type BoxedNewPeerFuture = Box<Future<Item=Peer, Error=Box<std::error::Error>>>;
pub type BoxedNewPeerStream = Box<Stream<Item=Peer, Error=Box<std::error::Error>>>;

pub trait Specifier {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor;
    fn is_stdio(&self) -> bool { false }
    /// "Directly" means "without reuser"
    fn directly_uses_stdio(&self) -> bool { false }
    fn is_multiconnect(&self) -> bool;
}

impl Specifier for Box<Specifier> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        (**self).construct(h, ps)
    }
    fn is_stdio(&self) -> bool { (**self).is_stdio() }
    fn directly_uses_stdio(&self) -> bool { (**self).directly_uses_stdio() }
    fn is_multiconnect(&self) -> bool { (**self).is_multiconnect() }
}

pub struct Stdio;
impl Specifier for Stdio {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let ret;
        #[cfg(all(unix,not(feature="no_unix_stdio")))]
        {
            ret = stdio_peer::get_stdio_peer(&mut ps.stdio, h)
        }
        #[cfg(any(not(unix),feature="no_unix_stdio"))]
        {
            ret = stdio_threaded_peer::get_stdio_peer()
        }
        once(ret)
    }
    fn is_stdio(&self) -> bool { true }
    fn directly_uses_stdio(&self) -> bool { true }
    fn is_multiconnect(&self) -> bool { false }
}

pub struct ThreadedStdio;
impl Specifier for ThreadedStdio {
    fn construct(&self, _:&Handle, _: &mut ProgramState) -> PeerConstructor {
        let ret;
        ret = stdio_threaded_peer::get_stdio_peer();
        once(ret)
    }
    fn is_stdio(&self) -> bool { true }
    fn directly_uses_stdio(&self) -> bool { true }
    fn is_multiconnect(&self) -> bool { false }
}


pub struct WsConnect<T:Specifier>(Url,T);
impl<T:Specifier> Specifier for WsConnect<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let inner = self.1.construct(h, ps);
        
        let url = self.0.clone();
        
        inner.map(move |q| {
            ws_client_peer::get_ws_client_peer_wrapped(&url, q)
        })
    }
    fn directly_uses_stdio(&self) -> bool { self.1.directly_uses_stdio() }
    fn is_multiconnect(&self) -> bool { self.1.is_multiconnect() }
}

pub struct WsUpgrade<T:Specifier>(T);
impl<T:Specifier> Specifier for WsUpgrade<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let inner = self.0.construct(h, ps);
        inner.map(ws_server_peer::ws_upgrade_peer)
    }
    fn directly_uses_stdio(&self) -> bool { self.0.directly_uses_stdio() }
    fn is_multiconnect(&self) -> bool { self.0.is_multiconnect() }
}

pub struct WsClient(Url);
impl Specifier for WsClient {
    fn construct(&self, h:&Handle, _: &mut ProgramState) -> PeerConstructor {
        let url = self.0.clone();
        once(ws_client_peer::get_ws_client_peer(h, &url))
    }
    fn is_multiconnect(&self) -> bool { false }
}

pub struct TcpConnect(SocketAddr);
impl Specifier for TcpConnect {
    fn construct(&self, h:&Handle, _: &mut ProgramState) -> PeerConstructor {
        once(net_peer::tcp_connect_peer(h, &self.0))
    }
    fn is_multiconnect(&self) -> bool { false }
}

pub struct TcpListen(SocketAddr);
impl Specifier for TcpListen {
    fn construct(&self, h:&Handle, _: &mut ProgramState) -> PeerConstructor {
        multi(net_peer::tcp_listen_peer(h, &self.0))
    }
    fn is_multiconnect(&self) -> bool { true }
}

pub struct Reuser<T:Specifier>(T);
impl<T:Specifier> Specifier for Reuser<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let mut reuser = ps.reuser.clone();
        let inner = self.0.construct(h, ps).get_only_first_conn();
        once(connection_reuse_peer::connection_reuser(&mut reuser, inner))
    }
    fn directly_uses_stdio(&self) -> bool { false }
    fn is_multiconnect(&self) -> bool { false }
}

pub enum PeerConstructor {
    ServeOnce(BoxedNewPeerFuture),
    ServeMultipleTimes(BoxedNewPeerStream),
}

impl PeerConstructor {
    pub fn map<F:'static>(self, f : F) -> Self
            where F:FnMut(Peer) -> BoxedNewPeerFuture
    {
        use PeerConstructor::*;
        match self {
            ServeOnce(x) => ServeOnce(Box::new(x.and_then(f)) as BoxedNewPeerFuture),
            ServeMultipleTimes(s) => ServeMultipleTimes(
                Box::new(
                    s.and_then(f)
                ) as BoxedNewPeerStream
            )
        }
    }
    
    pub fn get_only_first_conn(self) -> BoxedNewPeerFuture {
        use PeerConstructor::*;
        match self {
            ServeMultipleTimes(stre) => {
                Box::new(
                    stre
                    .into_future()
                    .map(move |(std_peer,_)| {
                        let peer2 = std_peer.expect("Nowhere to connect it");
                        peer2
                    })
                    .map_err(|(e,_)|e)
                ) as BoxedNewPeerFuture
            },
            ServeOnce(future) => {
                future
            },
        }
    }
}

pub fn once(x:BoxedNewPeerFuture) -> PeerConstructor {
    PeerConstructor::ServeOnce(x)
}
pub fn multi(x:BoxedNewPeerStream) -> PeerConstructor {
    PeerConstructor::ServeMultipleTimes(x)
}

pub fn peer_err<E: std::error::Error + 'static>(e : E) -> BoxedNewPeerFuture {
    Box::new(futures::future::err(Box::new(e) as Box<std::error::Error>)) as BoxedNewPeerFuture
}
pub fn peer_err_s<E: std::error::Error + 'static>(e : E) -> BoxedNewPeerStream {
    Box::new(
        futures::stream::iter_result(vec![Err(Box::new(e) as Box<std::error::Error>)])
    ) as BoxedNewPeerStream
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

pub mod connection_reuse_peer;

impl Peer {
    fn new<R:AsyncRead+'static, W:AsyncWrite+'static>(r:R, w:W) -> Self {
        Peer (
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
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

pub fn reuser_prefix(s:&str) -> Option<&str> {
    if s.starts_with("reuse:") {
        Some(&s[6..])
    } else {
        None
    }
}

fn boxup<T:Specifier+'static>(x:T) -> Result<Box<Specifier>> {
    Ok(Box::new(x))
}

pub fn spec(s : &str) -> Result<Box<Specifier>>  {
    FromStr::from_str(s)
}

impl FromStr for Box<Specifier> {
    type Err = Box<std::error::Error>;
    
    fn from_str(s: &str) -> Result<Box<Specifier>> {
            if s == "-" || s == "inetd:" {
            boxup(Stdio)
        } else 
        if s == "threadedstdio:" {
            boxup(ThreadedStdio)
        } else
        if s.starts_with("tcp:") {
            boxup(TcpConnect(s[4..].parse()?))
        } else 
        if s.starts_with("tcp-connect:") {
            boxup(TcpConnect(s[12..].parse()?))
        } else 
        if s.starts_with("tcp-l:") {
            boxup(TcpListen(s[6..].parse()?))
        } else 
        if s.starts_with("l-tcp:") {
            boxup(TcpListen(s[6..].parse()?))
        } else 
        if s.starts_with("tcp-listen:") {
            boxup(TcpListen(s[11..].parse()?))
        } else
        if let Some(x) = ws_l_prefix(s) {
            if x == "" {
                Err("Specify underlying protocol for ws-l:")?;
            }
            if let Some(c) = x.chars().next() {
                if c.is_numeric() || c == '[' {
                    // Assuming user uses old format like ws-l:127.0.0.1:8080
                    return spec(&("ws-l:tcp-l:".to_owned() + x));
                }
            }
            boxup(WsUpgrade(spec(x)?))
        } else
        if let Some(x) = ws_c_prefix(s) {
            boxup(WsConnect(Url::parse("ws://0.0.0.0/").unwrap(), spec(x)?))
        } else
        if let Some(x) = reuser_prefix(s) {
            boxup(Reuser(spec(x)?))
        } else
        {
            let url : Url = s.parse()?;
            boxup(WsClient(url))
        }
    }
}

pub fn peer_from_str(ps: &mut ProgramState, handle: &Handle, s: &str) -> PeerConstructor {
    let spec = match spec(s) {
        Ok(x) => x,
        Err(e) => return once(Box::new(futures::future::err(e)) as BoxedNewPeerFuture),
    };
    spec.construct(handle, ps)
}

pub struct Transfer {
    from: Box<AsyncRead>,
    to:   Box<AsyncWrite>,
}
pub struct Session(Transfer,Transfer);


impl Session {
    pub fn run(self) -> Box<Future<Item=(),Error=Box<std::error::Error>>> {
        let f1 = my_copy::copy(self.0.from, self.0.to, true);
        let f2 = my_copy::copy(self.1.from, self.1.to, true);
        Box::new(
            f1.join(f2)
            .map(|(_,_)|())
            .map_err(|x|  Box::new(x) as Box<std::error::Error> )
        ) as Box<Future<Item=(),Error=Box<std::error::Error>>>
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

#[derive(Default)]
pub struct Options {
}

pub fn serve<S1, S2, OE>(h: Handle, mut ps: ProgramState, s1: S1, s2 : S2, _options: Options, onerror: std::rc::Rc<OE>) 
    -> Box<Future<Item=(), Error=()>>
    where S1 : Specifier + 'static, S2: Specifier + 'static, OE : Fn(Box<std::error::Error>) -> () + 'static
{
    use PeerConstructor::{ServeMultipleTimes, ServeOnce};

    let h1 = h.clone();
    let h2 = h.clone();
    
    let e1 = onerror.clone();
    let e2 = onerror.clone();
    let e3 = onerror.clone();

    let left = s1.construct(&h, &mut ps);
    let prog = match left {
        ServeMultipleTimes(stream) => {
            let runner = stream
            .map(move |peer1| {
                let e1_1 = e1.clone();
                h1.spawn(
                    s2.construct(&h1, &mut ps)
                    .get_only_first_conn()
                    .and_then(move |peer2| {
                        let s = Session::new(peer1,peer2);
                        s.run()
                    })
                    .map_err(move|e|e1_1(e))
                )
            }).for_each(|()|futures::future::ok(()));
            Box::new(runner.map_err(move|e|e2(e))) as Box<Future<Item=(), Error=()>>
        },
        ServeOnce(peer1c) => {
            let runner = peer1c
            .and_then(move |peer1| {
                let right = s2.construct(&h2, &mut ps);
                let fut = right.get_only_first_conn();
                fut.and_then(move |peer2| {
                    let s = Session::new(peer1,peer2);
                    s.run().map(|()| {
                        ::std::mem::drop(ps) 
                        // otherwise ps will be dropped sooner
                        // and stdin/stdout may become blocking sooner
                    })
                })
            });
            Box::new(runner.map_err(move |e|e3(e))) as Box<Future<Item=(), Error=()>>
        },
    };
    prog
}
    
