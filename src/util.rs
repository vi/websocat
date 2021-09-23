use super::{
    futures, AsyncRead, AsyncWrite, BoxedNewPeerFuture, BoxedNewPeerStream, L2rUser, Peer,
    PeerConstructor, Rc, HupToken,
};
use super::{Future, Stream};

pub fn wouldblock<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, ""))
}
pub fn brokenpipe<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))
}
pub fn io_other_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

#[cfg_attr(feature = "cargo-clippy", allow(redundant_closure))]
impl PeerConstructor {
    pub fn map<F: 'static>(self, func: F) -> Self
    where
        F: Fn(Peer, L2rUser) -> BoxedNewPeerFuture,
    {
        let f = Rc::new(func);
        use crate::PeerConstructor::*;
        match self {
            Error(e) => Error(e),
            ServeOnce(x) => Overlay1(x, f),
            ServeMultipleTimes(s) => OverlayM(s, f),
            Overlay1(x, mapper) => Overlay1(
                x,
                Rc::new(move |p, l2r| {
                    let ff = f.clone();
                    let l2rc = l2r.clone();
                    Box::new(mapper(p, l2r).and_then(move |x| ff(x, l2rc)))
                }),
            ),
            OverlayM(x, mapper) => OverlayM(
                x,
                Rc::new(move |p, l2r| {
                    let ff = f.clone();
                    let l2rc = l2r.clone();
                    Box::new(mapper(p, l2r).and_then(move |x| ff(x, l2rc)))
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

    pub fn get_only_first_conn(self, l2r: L2rUser) -> BoxedNewPeerFuture {
        use crate::PeerConstructor::*;
        match self {
            Error(e) => Box::new(futures::future::err(e)) as BoxedNewPeerFuture,
            ServeMultipleTimes(stre) => Box::new(
                stre.into_future()
                    .map(move |(std_peer, _)| std_peer.expect("Nowhere to connect it"))
                    .map_err(|(e, _)| e),
            ) as BoxedNewPeerFuture,
            ServeOnce(futur) => futur,
            Overlay1(futur, mapper) => {
                Box::new(futur.and_then(move |p| mapper(p, l2r))) as BoxedNewPeerFuture
            }
            OverlayM(stre, mapper) => Box::new(
                stre.into_future()
                    .map(move |(std_peer, _)| std_peer.expect("Nowhere to connect it"))
                    .map_err(|(e, _)| e)
                    .and_then(move |p| mapper(p, l2r)),
            ) as BoxedNewPeerFuture,
        }
    }
}

pub fn once(x: BoxedNewPeerFuture) -> PeerConstructor {
    PeerConstructor::ServeOnce(x)
}
pub fn multi(x: BoxedNewPeerStream) -> PeerConstructor {
    PeerConstructor::ServeMultipleTimes(x)
}

pub fn peer_err<E: std::error::Error + 'static>(e: E) -> BoxedNewPeerFuture {
    Box::new(futures::future::err(
        Box::new(e) as Box<dyn std::error::Error>
    )) as BoxedNewPeerFuture
}
pub fn peer_err2(e: Box<dyn std::error::Error>) -> BoxedNewPeerFuture {
    Box::new(futures::future::err(
        e
    )) as BoxedNewPeerFuture
}
pub fn peer_err_s<E: std::error::Error + 'static>(e: E) -> BoxedNewPeerStream {
    Box::new(futures::stream::iter_result(vec![Err(
        Box::new(e) as Box<dyn std::error::Error>
    )])) as BoxedNewPeerStream
}
pub fn peer_err_sb(e: Box<dyn std::error::Error + 'static>) -> BoxedNewPeerStream {
    Box::new(futures::stream::iter_result(vec![Err(
        e
    )])) as BoxedNewPeerStream
}
pub fn peer_strerr(e: &str) -> BoxedNewPeerFuture {
    let q: Box<dyn std::error::Error> = From::from(e);
    Box::new(futures::future::err(q)) as BoxedNewPeerFuture
}
pub fn simple_err(e: String) -> std::io::Error {
    let e1: Box<dyn std::error::Error + Send + Sync> = e.into();
    ::std::io::Error::new(::std::io::ErrorKind::Other, e1)
}
pub fn simple_err2(e: &'static str) -> Box<dyn std::error::Error> {
    let e1: Box<dyn std::error::Error + Send + Sync> = e.to_string().into();
    e1 as Box<dyn std::error::Error>
}
pub fn box_up_err<E: std::error::Error + 'static>(e: E) -> Box<dyn std::error::Error> {
    Box::new(e) as Box<dyn std::error::Error>
}

impl Peer {
    pub fn new<R: AsyncRead + 'static, W: AsyncWrite + 'static>(r: R, w: W, hup: Option<HupToken>) -> Self {
        Peer(
            Box::new(r) as Box<dyn AsyncRead>,
            Box::new(w) as Box<dyn AsyncWrite>,
            hup,
        )
    }
}
