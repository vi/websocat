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
    stdio : stdio_peer::GlobalState,
}

pub struct Peer(Box<AsyncRead>, Box<AsyncWrite>);
type BoxedNewPeerFuture = Box<Future<Item=Peer, Error=Box<std::error::Error>>>;

mod my_copy;

pub mod ws_peer;
pub mod stdio_peer;

impl Peer {
    fn new<R:AsyncRead+'static, W:AsyncWrite+'static>(r:R, w:W) -> Self {
        Peer (
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
    }
}

pub fn peer_from_str(ps: &mut ProgramState, handle: &Handle, s: &str) -> BoxedNewPeerFuture {
    if s == "-" {
        stdio_peer::get_stdio_peer(&mut ps.stdio, handle)
    } else {
        ws_peer::get_ws_client_peer(handle, s)
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

