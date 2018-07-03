//! Note: library usage is not semver/API-stable
//!
//! Type evolution of a websocat run:
//!
//! 1. `&str` - string as passed to command line. When it meets the list of `SpecifierClass`es, there appears:
//! 2. `Specifier` - more organized representation, may be nested. When `construct` is called, we get:
//! 3. `PeerConstructor` - a future or stream that returns one or more connections. After completion, we get one or more of:
//! 4. `Peer` - an active connection. Once we have two of them, we can start a:
//! 5. `Session` with two `Transfer`s - forward and reverse.

extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate websocket;

#[macro_use]
extern crate log;

#[macro_use]
extern crate slab_typesafe;

#[macro_use]
extern crate smart_default;

use futures::future::Future;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

use futures::Stream;

use std::cell::RefCell;
use std::rc::Rc;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn wouldblock<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, ""))
}
fn brokenpipe<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))
}
fn io_other_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

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
            s1: Specifier::from_stack(self.s1)?,
            s2: Specifier::from_stack(self.s2)?,
        })
    }
}

pub struct WebsocatConfiguration3 {
    pub opts: Options,
    pub s1: Rc<Specifier>,
    pub s2: Rc<Specifier>,
}

impl WebsocatConfiguration3 {
    pub fn serve<OE>(
        self,
        h: Handle,
        onerror: std::rc::Rc<OE>,
    ) -> Box<Future<Item = (), Error = ()>>
    where
        OE: Fn(Box<std::error::Error>) -> () + 'static,
    {
        serve(h, self.s1, self.s2, self.opts, onerror)
    }
}

#[derive(SmartDefault, Debug, Clone)]
pub struct Options {
    pub websocket_text_mode: bool,
    pub websocket_protocol: Option<String>,
    pub udp_oneshot_mode: bool,
    pub unidirectional: bool,
    pub unidirectional_reverse: bool,
    pub exit_on_eof: bool,
    pub oneshot: bool,
    pub unlink_unix_socket: bool,
    pub exec_args: Vec<String>,
    pub ws_c_uri: String,
    pub linemode_strip_newlines: bool,
    pub linemode_strict: bool,
    pub origin: Option<String>,
    pub custom_headers: Vec<(String, Vec<u8>)>,
    pub websocket_version: Option<String>,
    pub websocket_dont_close: bool,
    pub one_message: bool,
    pub no_auto_linemode: bool,
    #[default = "65536"]
    pub buffer_size: usize,
    #[default = "16"]
    pub broadcast_queue_len : usize,
    #[default = "readdebt::DebtHandling::Silent"]
    pub read_debt_handling : readdebt::DebtHandling,
}

#[derive(Default)]
pub struct ProgramState {
    #[cfg(all(unix, feature = "unix_stdio"))]
    stdio: stdio_peer::GlobalState,

    reuser: primitive_reuse_peer::GlobalState,
    reuser2: broadcast_reuse_peer::GlobalState,
}

/// Some information passed from the left specifier Peer to the right
#[derive(Default, Clone)]
pub struct LeftSpecToRightSpec {}
#[derive(Clone)]
pub enum L2rUser {
    FillIn(Rc<RefCell<LeftSpecToRightSpec>>),
    ReadFrom(Rc<RefCell<LeftSpecToRightSpec>>),
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);

pub type BoxedNewPeerFuture = Box<Future<Item = Peer, Error = Box<std::error::Error>>>;
pub type BoxedNewPeerStream = Box<Stream<Item = Peer, Error = Box<std::error::Error>>>;

#[macro_use]
pub mod specifier;
pub use specifier::{ClassMessageBoundaryStatus, ClassMulticonnectStatus, SpecifierClass, SpecifierStack, Specifier, ConstructParams};

#[macro_use]
pub mod all_peers;

pub mod lints;
mod my_copy;

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
pub mod line_peer;
pub mod primitive_reuse_peer;
pub mod reconnect_peer;

pub mod specparse;

pub type PeerOverlay = Rc<Fn(Peer) -> BoxedNewPeerFuture>;

pub enum PeerConstructor {
    ServeOnce(BoxedNewPeerFuture),
    ServeMultipleTimes(BoxedNewPeerStream),
    Overlay1(BoxedNewPeerFuture, PeerOverlay),
    OverlayM(BoxedNewPeerStream, PeerOverlay),
}

#[cfg_attr(feature = "cargo-clippy", allow(redundant_closure))]
impl PeerConstructor {
    pub fn map<F: 'static>(self, func: F) -> Self
    where
        F: Fn(Peer) -> BoxedNewPeerFuture,
    {
        let f = Rc::new(func);
        use PeerConstructor::*;
        match self {
            ServeOnce(x) => Overlay1(x, f),
            ServeMultipleTimes(s) => OverlayM(s, f),
            Overlay1(x, mapper) => Overlay1(
                x,
                Rc::new(move |p| {
                    let ff = f.clone();
                    Box::new(mapper(p).and_then(move |x| ff(x)))
                }),
            ),
            OverlayM(x, mapper) => OverlayM(
                x,
                Rc::new(move |p| {
                    let ff = f.clone();
                    Box::new(mapper(p).and_then(move |x| ff(x)))
                }),
            ), // This implementation (without Overlay{1,M} cases)
            // causes task to be spawned too late (before establishing ws upgrade)
            // when serving clients:
            
            //ServeOnce(x) => ServeOnce(Box::new(x.and_then(f)) as BoxedNewPeerFuture),
            //ServeMultipleTimes(s) => {
            //    ServeMultipleTimes(Box::new(s.and_then(f)) as BoxedNewPeerStream)
            //}
        }
    }

    pub fn get_only_first_conn(self) -> BoxedNewPeerFuture {
        use PeerConstructor::*;
        match self {
            ServeMultipleTimes(stre) => Box::new(
                stre.into_future()
                    .map(move |(std_peer, _)| std_peer.expect("Nowhere to connect it"))
                    .map_err(|(e, _)| e),
            ) as BoxedNewPeerFuture,
            ServeOnce(futur) => futur,
            Overlay1(futur, mapper) => {
                Box::new(futur.and_then(move |p| mapper(p))) as BoxedNewPeerFuture
            }
            OverlayM(stre, mapper) => Box::new(
                stre.into_future()
                    .map(move |(std_peer, _)| std_peer.expect("Nowhere to connect it"))
                    .map_err(|(e, _)| e)
                    .and_then(move |p| mapper(p)),
            ) as BoxedNewPeerFuture,
        }
    }
}

pub mod readdebt;

pub fn once(x: BoxedNewPeerFuture) -> PeerConstructor {
    PeerConstructor::ServeOnce(x)
}
pub fn multi(x: BoxedNewPeerStream) -> PeerConstructor {
    PeerConstructor::ServeMultipleTimes(x)
}

pub fn peer_err<E: std::error::Error + 'static>(e: E) -> BoxedNewPeerFuture {
    Box::new(futures::future::err(Box::new(e) as Box<std::error::Error>)) as BoxedNewPeerFuture
}
pub fn peer_err_s<E: std::error::Error + 'static>(e: E) -> BoxedNewPeerStream {
    Box::new(futures::stream::iter_result(vec![Err(
        Box::new(e) as Box<std::error::Error>
    )])) as BoxedNewPeerStream
}
pub fn peer_strerr(e: &str) -> BoxedNewPeerFuture {
    let q: Box<std::error::Error> = From::from(e);
    Box::new(futures::future::err(q)) as BoxedNewPeerFuture
}
pub fn simple_err(e: String) -> std::io::Error {
    let e1: Box<std::error::Error + Send + Sync> = e.into();
    ::std::io::Error::new(::std::io::ErrorKind::Other, e1)
}
pub fn box_up_err<E: std::error::Error + 'static>(e: E) -> Box<std::error::Error> {
    Box::new(e) as Box<std::error::Error>
}

impl Peer {
    fn new<R: AsyncRead + 'static, W: AsyncWrite + 'static>(r: R, w: W) -> Self {
        Peer(
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
    }
}

pub use specparse::spec;

pub fn peer_from_str(
    ps: Rc<RefCell<ProgramState>>,
    handle: Handle,
    opts: Rc<Options>,
    s: &str,
) -> PeerConstructor {
    let spec = match spec(s) {
        Ok(x) => x,
        Err(e) => return once(Box::new(futures::future::err(e)) as BoxedNewPeerFuture),
    };
    let l2r = Rc::new(RefCell::new(Default::default()));
    let cp = ConstructParams {
        tokio_handle: handle,
        program_options: opts,
        global_state: ps,
        left_to_right: L2rUser::ReadFrom(l2r),
    };
    spec.construct(cp)
}

pub struct Transfer {
    from: Box<AsyncRead>,
    to: Box<AsyncWrite>,
}
pub struct Session(Transfer, Transfer, Rc<Options>);

impl Session {
    pub fn run(self) -> Box<Future<Item = (), Error = Box<std::error::Error>>> {
        let once = self.2.one_message;
        let co = my_copy::CopyOptions {
            stop_on_reader_zero_read: true,
            once,
            buffer_size: self.2.buffer_size,
        };
        let f1 = my_copy::copy(self.0.from, self.0.to, co);
        let f2 = my_copy::copy(self.1.from, self.1.to, co);
        // TODO: properly shutdown in unidirectional mode
        let f1 = f1.and_then(|(_, r, w)| {
            info!("Forward finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                info!("Forward shutdown finished");
                std::mem::drop(w);
            })
        });
        let f2 = f2.and_then(|(_, r, w)| {
            info!("Reverse finished");
            std::mem::drop(r);
            tokio_io::io::shutdown(w).map(|w| {
                info!("Reverse shutdown finished");
                std::mem::drop(w);
            })
        });
        
        let (unif, unir, eeof) = (
            self.2.unidirectional,
            self.2.unidirectional_reverse,
            self.2.exit_on_eof,
        );
        type Ret = Box<Future<Item = (), Error = Box<std::error::Error>>>;
        match (unif, unir, eeof) {
            (false, false, false) => Box::new(
                f1.join(f2)
                    .map(|(_, _)| {
                        info!("Finished");
                    })
                    .map_err(|x| Box::new(x) as Box<std::error::Error>),
            ) as Ret,
            (false, false, true) => Box::new(
                f1.select(f2)
                    .map(|(_, _)| {
                        info!("One of directions finished");
                    })
                    .map_err(|(x, _)| Box::new(x) as Box<std::error::Error>),
            ) as Ret,
            (true, false, _) => Box::new({
                ::std::mem::drop(f2);
                f1.map_err(|x| Box::new(x) as Box<std::error::Error>)
            }) as Ret,
            (false, true, _) => Box::new({
                ::std::mem::drop(f1);
                f2.map_err(|x| Box::new(x) as Box<std::error::Error>)
            }) as Ret,
            (true, true, _) => Box::new({
                // Just open connection and close it.
                ::std::mem::drop(f1);
                ::std::mem::drop(f2);
                futures::future::ok(())
            }) as Ret,
        }
    }
    pub fn new(peer1: Peer, peer2: Peer, opts: Rc<Options>) -> Self {
        Session(
            Transfer {
                from: peer1.0,
                to: peer2.1,
            },
            Transfer {
                from: peer2.0,
                to: peer1.1,
            },
            opts,
        )
    }
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
pub fn serve<OE>(
    h: Handle,
    s1: Rc<Specifier>,
    s2: Rc<Specifier>,
    opts: Options,
    onerror: std::rc::Rc<OE>,
) -> Box<Future<Item = (), Error = ()>>
where
    OE: Fn(Box<std::error::Error>) -> () + 'static,
{
    info!("Serving {:?} to {:?} with {:?}", s1, s2, opts);
    let ps = Rc::new(RefCell::new(ProgramState::default()));

    use PeerConstructor::{Overlay1, OverlayM, ServeMultipleTimes, ServeOnce};

    let h1 = h.clone();

    let e1 = onerror.clone();
    let e2 = onerror.clone();
    let e3 = onerror.clone();

    let opts1 = Rc::new(opts);
    let opts2 = opts1.clone();

    let l2r = Rc::new(RefCell::new(Default::default()));
    let cp1 = ConstructParams {
        tokio_handle: h.clone(),
        program_options: opts1.clone(),
        global_state: ps.clone(),
        left_to_right: L2rUser::FillIn(l2r.clone()),
    };
    let cp2 = ConstructParams {
        tokio_handle: h.clone(),
        program_options: opts1,
        global_state: ps.clone(),
        left_to_right: L2rUser::ReadFrom(l2r),
    };
    let mut left = s1.construct(cp1);

    if opts2.oneshot {
        left = PeerConstructor::ServeOnce(left.get_only_first_conn());
    }

    match left {
        ServeMultipleTimes(stream) => {
            let runner = stream
                .map(move |peer1| {
                    let opts3 = opts2.clone();
                    let e1_1 = e1.clone();
                    let cp2 = cp2.clone();
                    h1.spawn(
                        s2.construct(cp2)
                            .get_only_first_conn()
                            .and_then(move |peer2| {
                                let s = Session::new(peer1, peer2, opts3);
                                s.run()
                            })
                            .map_err(move |e| e1_1(e)),
                    )
                })
                .for_each(|()| futures::future::ok(()));
            Box::new(runner.map_err(move |e| e2(e))) as Box<Future<Item = (), Error = ()>>
        }
        OverlayM(stream, mapper) => {
            let runner = stream
                .map(move |peer1_| {
                    debug!("Underlying connection established");
                    let opts3 = opts2.clone();
                    let e1_1 = e1.clone();
                    let s2 = s2.clone();
                    let h1 = h1.clone();
                    let cp2 = cp2.clone();
                    h1.spawn(
                        mapper(peer1_)
                            .and_then(move |peer1| {
                                s2.construct(cp2)
                                    .get_only_first_conn()
                                    .and_then(move |peer2| {
                                        let s = Session::new(peer1, peer2, opts3);
                                        s.run()
                                    })
                            })
                            .map_err(move |e| e1_1(e)),
                    )
                })
                .for_each(|()| futures::future::ok(()));
            Box::new(runner.map_err(move |e| e2(e))) as Box<Future<Item = (), Error = ()>>
        }
        ServeOnce(peer1c) => {
            let runner = peer1c.and_then(move |peer1| {
                let right = s2.construct(cp2);
                let fut = right.get_only_first_conn();
                fut.and_then(move |peer2| {
                    let s = Session::new(peer1, peer2, opts2);
                    s.run().map(|()| {
                        ::std::mem::drop(ps)
                        // otherwise ps will be dropped sooner
                        // and stdin/stdout may become blocking sooner
                    })
                })
            });
            Box::new(runner.map_err(move |e| e3(e))) as Box<Future<Item = (), Error = ()>>
        }
        Overlay1(peer1c, mapper) => {
            let runner = peer1c.and_then(move |peer1_| {
                debug!("Underlying connection established");
                mapper(peer1_).and_then(move |peer1| {
                    let right = s2.construct(cp2);
                    let fut = right.get_only_first_conn();
                    fut.and_then(move |peer2| {
                        let s = Session::new(peer1, peer2, opts2);
                        s.run().map(|()| {
                            ::std::mem::drop(ps)
                            // otherwise ps will be dropped sooner
                            // and stdin/stdout may become blocking sooner
                        })
                    })
                })
            });
            Box::new(runner.map_err(move |e| e3(e))) as Box<Future<Item = (), Error = ()>>
        }
    }
}
