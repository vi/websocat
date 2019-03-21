//! Note: library usage is not semver/API-stable
//!
//! Type evolution of a websocat run:
//!
//! 1. `&str` - string as passed to command line. When it meets the list of `SpecifierClass`es, there appears:
//! 2. `SpecifierStack` - specifier class, final string argument and vector of overlays.
//! 3. `Specifier` - more rigid version of SpecifierStack, with everything parsable parsed. May be nested. When `construct` is called, we get:
//! 4. `PeerConstructor` - a future or stream that returns one or more connections. After completion, we get one or more of:
//! 5. `Peer` - an active connection. Once we have two of them, we can start a:
//! 6. `Session` with two `Transfer`s - forward and reverse.

#![allow(renamed_and_removed_lints)]
#![allow(unknown_lints)]
#![cfg_attr(feature = "cargo-clippy", allow(deprecated_cfg_attr))]

extern crate futures;
#[macro_use]
extern crate tokio_io;
extern crate tokio_current_thread;
extern crate tokio_reactor;
extern crate tokio_tcp;
extern crate tokio_udp;
extern crate websocket;

extern crate tk_listen;

#[macro_use]
extern crate log;

#[macro_use]
extern crate slab_typesafe;

#[macro_use]
extern crate smart_default;
#[macro_use]
extern crate derivative;

extern crate tokio_timer;

use futures::future::Future;
use tokio_io::{AsyncRead, AsyncWrite};

use futures::Stream;

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

pub struct WebsocatConfiguration1 {
    pub opts: Options,
    pub addr1: String,
    pub addr2: String,
}

impl WebsocatConfiguration1 {
    pub fn parse1(self) -> Result<WebsocatConfiguration2> {
        Ok(WebsocatConfiguration2 {
            opts: self.opts,
            s1: SpecifierStack::from_str(self.addr1.as_str())?,
            s2: SpecifierStack::from_str(self.addr2.as_str())?,
        })
    }
}

pub struct WebsocatConfiguration2 {
    pub opts: Options,
    pub s1: SpecifierStack,
    pub s2: SpecifierStack,
}

impl WebsocatConfiguration2 {
    pub fn parse2(self) -> Result<WebsocatConfiguration3> {
        Ok(WebsocatConfiguration3 {
            opts: self.opts,
            s1: Specifier::from_stack(&self.s1)?,
            s2: Specifier::from_stack(&self.s2)?,
        })
    }
}

pub struct WebsocatConfiguration3 {
    pub opts: Options,
    pub s1: Rc<Specifier>,
    pub s2: Rc<Specifier>,
}

impl WebsocatConfiguration3 {
    pub fn serve<OE>(self, onerror: std::rc::Rc<OE>) -> impl Future<Item = (), Error = ()>
    where
        OE: Fn(Box<std::error::Error>) -> () + 'static,
    {
        serve(self.s1, self.s2, self.opts, onerror)
    }
}

pub mod options;
pub use options::Options;

#[derive(Default)]
pub struct ProgramState {
    #[cfg(all(unix, feature = "unix_stdio"))]
    stdio: stdio_peer::GlobalState,

    reuser: primitive_reuse_peer::GlobalState,
    reuser2: broadcast_reuse_peer::GlobalState,
}

/// Some information passed from the left specifier Peer to the right
#[derive(Default, Clone)]
pub struct LeftSpecToRightSpec {
    /// URI the client requested when connecting to WebSocket
    uri: Option<String>,
    /// Address:port of connecting client, if it is TCP
    client_addr: Option<String>,
}

pub type L2rWriter = Rc<RefCell<LeftSpecToRightSpec>>;
pub type L2rReader = Rc<LeftSpecToRightSpec>;

#[derive(Clone)]
pub enum L2rUser {
    FillIn(L2rWriter),
    ReadFrom(L2rReader),
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);

pub type BoxedNewPeerFuture = Box<Future<Item = Peer, Error = Box<std::error::Error>>>;
pub type BoxedNewPeerStream = Box<Stream<Item = Peer, Error = Box<std::error::Error>>>;

#[macro_use]
pub mod specifier;
pub use specifier::{
    ClassMessageBoundaryStatus, ClassMulticonnectStatus, ConstructParams, Specifier,
    SpecifierClass, SpecifierStack,
};

#[macro_use]
pub mod all_peers;

pub mod lints;
mod my_copy;

pub use util::{brokenpipe, io_other_error, simple_err2, wouldblock};

#[cfg(all(unix, feature = "unix_stdio"))]
pub mod stdio_peer;

pub mod file_peer;
pub mod mirror_peer;
pub mod net_peer;
pub mod stdio_threaded_peer;
pub mod trivial_peer;
pub mod ws_client_peer;
pub mod ws_peer;
pub mod ws_server_peer;

#[cfg(feature = "tokio-process")]
pub mod process_peer;

#[cfg(unix)]
pub mod unix_peer;

pub mod broadcast_reuse_peer;
pub mod jsonrpc_peer;
pub mod line_peer;
pub mod primitive_reuse_peer;
pub mod reconnect_peer;

pub mod proxy_peer;
#[cfg(feature = "ssl")]
pub mod ssl_peer;

pub mod specparse;

pub type PeerOverlay = Rc<Fn(Peer, L2rUser) -> BoxedNewPeerFuture>;

pub enum PeerConstructor {
    ServeOnce(BoxedNewPeerFuture),
    ServeMultipleTimes(BoxedNewPeerStream),
    Overlay1(BoxedNewPeerFuture, PeerOverlay),
    OverlayM(BoxedNewPeerStream, PeerOverlay),
}

/// A remnant of the hack
pub fn spawn_hack<T>(f: T)
where
    T: Future<Item = (), Error = ()> + 'static,
{
    tokio_current_thread::TaskExecutor::current()
        .spawn_local(Box::new(f))
        .unwrap()
}

pub mod util;
pub use util::{box_up_err, multi, once, peer_err, peer_err_s, peer_strerr, simple_err};

pub mod readdebt;

pub use specparse::spec;

pub struct Transfer {
    from: Box<AsyncRead>,
    to: Box<AsyncWrite>,
}
pub struct Session(Transfer, Transfer, Rc<Options>);

pub mod sessionserve;
pub use sessionserve::serve;
