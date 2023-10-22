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
extern crate tokio_codec;
extern crate tokio_timer;
extern crate websocket;
extern crate websocket_base;
extern crate http_bytes;
extern crate anymap;
pub use http_bytes::http;

extern crate tk_listen;
extern crate net2;

#[macro_use]
extern crate log;

#[macro_use]
extern crate slab_typesafe;

#[macro_use]
extern crate smart_default;
#[macro_use]
extern crate derivative;

use futures::future::Future;
use tokio_io::{AsyncRead, AsyncWrite};

use futures::Stream;

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// First representation of websocat command-line, partially parsed.
pub struct WebsocatConfiguration1 {
    pub opts: Options,
    pub addr1: String,
    pub addr2: String,
}

impl WebsocatConfiguration1 {
    /// Is allowed to call blocking calls
    /// happens only at start of websocat
    pub fn parse1(self) -> Result<WebsocatConfiguration2> {
        Ok(WebsocatConfiguration2 {
            opts: self.opts,
            s1: SpecifierStack::from_str(self.addr1.as_str())?,
            s2: SpecifierStack::from_str(self.addr2.as_str())?,
        })
    }
}

/// Second representation of websocat configuration: everything
/// (e.g. socket addresses) should already be parsed and verified
/// A structural form: two chains of specifier nodes.
/// Futures/async is not yet involved at this stage, but everything
/// should be checked and ready to do to start it (apart from OS errors)
/// 
/// This form is designed to be editable by lints and command-line options.
pub struct WebsocatConfiguration2 {
    pub opts: Options,
    pub s1: SpecifierStack,
    pub s2: SpecifierStack,
}

impl WebsocatConfiguration2 {
    pub fn parse2(self) -> Result<WebsocatConfiguration3> {
        Ok(WebsocatConfiguration3 {
            opts: self.opts,
            s1: <dyn Specifier>::from_stack(&self.s1)?,
            s2: <dyn Specifier>::from_stack(&self.s2)?,
        })
    }
}

/// An immutable chain of functions that results in a `Future`s or `Streams` that rely on each other.
/// This is somewhat like a frozen form of `WebsocatConfiguration2`.
pub struct WebsocatConfiguration3 {
    pub opts: Options,
    pub s1: Rc<dyn Specifier>,
    pub s2: Rc<dyn Specifier>,
}

impl WebsocatConfiguration3 {
    pub fn serve<OE>(self, onerror: std::rc::Rc<OE>) -> impl Future<Item = (), Error = ()>
    where
        OE: Fn(Box<dyn std::error::Error>) -> () + 'static,
    {
        serve(self.s1, self.s2, self.opts, onerror)
    }
}

pub mod options;
pub use crate::options::Options;

#[derive(SmartDefault)]
pub struct ProgramState(
    #[default(anymap::AnyMap::with_capacity(2))]
    anymap::AnyMap
);

/// Some information passed from the left specifier Peer to the right
#[derive(Default, Clone)]
pub struct LeftSpecToRightSpec {
    /// URI the client requested when connecting to WebSocket
    uri: Option<String>,
    /// Address:port of connecting client, if it is TCP
    client_addr: Option<String>,
    /// All incoming HTTP headers
    headers: Vec<(String, String)>,
}

pub type L2rWriter = Rc<RefCell<LeftSpecToRightSpec>>;
pub type L2rReader = Rc<LeftSpecToRightSpec>;

#[derive(Clone)]
pub enum L2rUser {
    FillIn(L2rWriter),
    ReadFrom(L2rReader),
}

/// Resolves if/when TCP socket gets reset
pub type HupToken = Box<dyn Future<Item=(), Error=Box<dyn std::error::Error>>>;

pub struct Peer(Box<dyn AsyncRead>, Box<dyn AsyncWrite>, Option<HupToken>);

pub type BoxedNewPeerFuture = Box<dyn Future<Item = Peer, Error = Box<dyn std::error::Error>>>;
pub type BoxedNewPeerStream = Box<dyn Stream<Item = Peer, Error = Box<dyn std::error::Error>>>;

#[macro_use]
pub mod specifier;
pub use crate::specifier::{
    ClassMessageBoundaryStatus, ClassMulticonnectStatus, ConstructParams, Specifier,
    SpecifierClass, SpecifierStack,
};

#[macro_use]
pub mod all_peers;

pub mod lints;
mod my_copy;

pub use crate::util::{brokenpipe, io_other_error, simple_err2, wouldblock};

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
pub mod ws_lowlevel_peer;
pub mod http_peer;

#[cfg(feature = "tokio-process")]
pub mod process_peer;


#[cfg(all(windows,feature = "windows_named_pipes"))]
pub mod windows_np_peer;

#[cfg(unix)]
pub mod unix_peer;

pub mod broadcast_reuse_peer;
pub mod jsonrpc_peer;
pub mod timestamp_peer;
pub mod line_peer;
pub mod lengthprefixed_peer;
pub mod foreachmsg_peer;
pub mod primitive_reuse_peer;
pub mod reconnect_peer;

pub mod socks5_peer;
#[cfg(feature = "ssl")]
pub mod ssl_peer;

#[cfg(feature = "crypto_peer")]
pub mod crypto_peer;

#[cfg(feature = "prometheus_peer")]
pub mod prometheus_peer;

#[cfg(feature = "native_plugins")]
pub mod transform_peer;

#[cfg(feature = "wasm_plugins")]
pub mod wasm_transform_peer;

pub mod specparse;

pub type PeerOverlay = Rc<dyn Fn(Peer, L2rUser) -> BoxedNewPeerFuture>;

pub enum PeerConstructor {
    ServeOnce(BoxedNewPeerFuture),
    ServeMultipleTimes(BoxedNewPeerStream),
    Overlay1(BoxedNewPeerFuture, PeerOverlay),
    OverlayM(BoxedNewPeerStream, PeerOverlay),
    Error(Box<dyn std::error::Error>),
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
pub use crate::util::{box_up_err, multi, once, peer_err, peer_err_s, peer_strerr, simple_err};

pub mod readdebt;

pub use crate::specparse::spec;

pub struct Transfer {
    from: Box<dyn AsyncRead>,
    to: Box<dyn AsyncWrite>,
}
pub struct Session {
    t1: Transfer,
    t2: Transfer,
    opts: Rc<Options>,
    hup1: Option<HupToken>,
    hup2: Option<HupToken>,
}

pub mod sessionserve;
pub use crate::sessionserve::serve;
