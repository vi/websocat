#![allow(unused)]

extern crate websocket;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_stdin_stdout;

#[cfg(unix)]
extern crate tokio_file_unix;
#[cfg(unix)]
extern crate tokio_signal;

use std::thread;
use std::io::stdin;
use tokio_core::reactor::{Core, Handle};
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use websocket::stream::async::Stream as WsStream;
use futures::Async::{Ready, NotReady};

use tokio_io::io::copy;

use tokio_io::codec::FramedRead;
use std::fs::File;

#[cfg(unix)]
use tokio_file_unix::{File as UnixFile, StdFile};
#[cfg(unix)]
use std::os::unix::io::FromRawFd;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[cfg(feature="ssl")]
type WaitingForImplTraitFeature0 = tokio_io::codec::Framed<std::boxed::Box<websocket::async::Stream + std::marker::Send>, websocket::async::MessageCodec<websocket::OwnedMessage>>;
#[cfg(not(feature="ssl"))]
type WaitingForImplTraitFeature0 = tokio_io::codec::Framed<websocket::async::TcpStream, websocket::async::MessageCodec<websocket::OwnedMessage>>;
type WaitingForImplTraitFeature2 = futures::stream::SplitSink<WaitingForImplTraitFeature0>;
type WsSource = futures::stream::SplitStream<WaitingForImplTraitFeature0>;
type MultiProducerWsSink = Rc<RefCell<WaitingForImplTraitFeature2>>;


fn wouldblock<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, ""))
}
fn brokenpipe<T>() -> std::io::Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, ""))
}
fn io_other_error<E : std::error::Error + Send + Sync + 'static>(e:E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other,e)
}

mod my_copy;


struct WsReadWrapper {
    s: WsSource,
    pingreply : MultiProducerWsSink,
    debt: Option<Vec<u8>>,
}

impl AsyncRead for WsReadWrapper {

}

impl WsReadWrapper {
    fn process_message(&mut self, buf: &mut [u8], buf_in: &[u8]) -> std::result::Result<usize, std::io::Error> {
        let l = buf_in.len().min(buf.len());
        buf[..l].copy_from_slice(&buf_in[..l]);
        
        if l < buf_in.len() {
            self.debt = Some(buf_in[l..].to_vec());
        }
        
        Ok(l)
    }
}

impl Read for WsReadWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(debt) = self.debt.take() {
            return self.process_message(buf, debt.as_slice())
        }
        match self.s.poll().map_err(io_other_error)? {
            Ready(Some(OwnedMessage::Close(_))) => {
                brokenpipe()
            },
            Ready(None) => {
                brokenpipe()
            }
            Ready(Some(OwnedMessage::Ping(x))) => {
                let om = OwnedMessage::Pong(x);
                let mut sink = self.pingreply.borrow_mut();
                let mut proceed = false;
                // I'm not sure this is safe enough, RefCell-wise and Futures-wise
                // And pings and their replies are not tested yet
                match sink.start_send(om).map_err(io_other_error)? {
                    futures::AsyncSink::NotReady(_) => {
                        // drop the ping
                    },
                    futures::AsyncSink::Ready => {
                        proceed = true;
                    }
                }
                if proceed {
                    let _ = sink.poll_complete().map_err(io_other_error)?;
                }
                
                Ok(0)
            }
            Ready(Some(OwnedMessage::Pong(_))) => {
                Ok(0)
            }
            Ready(Some(OwnedMessage::Text(x))) => {
                self.process_message(buf, x.as_str().as_bytes())
            }
            Ready(Some(OwnedMessage::Binary(x))) => {
                self.process_message(buf, x.as_slice())
            }
            NotReady => {
                wouldblock()
            }
        }
    }
}

struct WsWriteWrapper(MultiProducerWsSink);

impl AsyncWrite for WsWriteWrapper {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        // TODO: check this
        Ok(Ready(()))
    }
}

impl Write for WsWriteWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = OwnedMessage::Binary(buf.to_vec());
        match self.0.borrow_mut().start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => {
                wouldblock()
            },
            futures::AsyncSink::Ready => {
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.borrow_mut().poll_complete().map_err(io_other_error)? {
            NotReady => {
                wouldblock()
            },
            Ready(()) => {
                Ok(())
            }
        }
    }
}

impl Drop for WsWriteWrapper {
    fn drop(&mut self) {
        let mut sink = self.0.borrow_mut();
        sink.start_send(OwnedMessage::Close(None))
            .map_err(|_|())
            .map(|_|());
        sink.poll_complete()
            .map_err(|_|())
            .map(|_|());
    }
}

struct Peer(Box<AsyncRead>, Box<AsyncWrite>);

impl Peer {
    fn new<R:AsyncRead+'static, W:AsyncWrite+'static>(r:R, w:W) -> Self {
        Peer (
            Box::new(r) as Box<AsyncRead>,
            Box::new(w) as Box<AsyncWrite>,
        )
    }
}

type BoxedNewPeerFuture = Box<Future<Item=Peer, Error=Box<std::error::Error>>>;

fn get_ws_client_peer(handle: &Handle, uri: &str) -> BoxedNewPeerFuture {
    let stage1 = match ClientBuilder::new(uri) {
        Ok(x) => x,
        Err(e) => return Box::new(futures::future::err(Box::new(e) as Box<std::error::Error>)),
    };
    let before_connect = stage1
        .add_protocol("rust-websocket");
    #[cfg(feature="ssl")]
    let after_connect = before_connect
        .async_connect(None, handle);
    #[cfg(not(feature="ssl"))]
    let after_connect = before_connect
        .async_connect_insecure(handle);
    Box::new(after_connect
        .map(|(duplex, _)| {
            let (sink, stream) = duplex.split();
            let mpsink = Rc::new(RefCell::new(sink));
            
            let ws_str = WsReadWrapper {
                s: stream,
                pingreply: mpsink.clone(),
                debt: None,
            };
            let ws_sin = WsWriteWrapper(mpsink);
            
            let ws = Peer::new(ws_str, ws_sin);
            ws
        })
        .map_err(|e|Box::new(e) as Box<std::error::Error>)
    ) as BoxedNewPeerFuture
}

fn get_stdio_peer_impl(handle: &Handle) -> Result<Peer> {
    let si;
    let so;
    
    #[cfg(any(not(unix),feature="no_unix_stdio"))]
    {
        si = tokio_stdin_stdout::stdin(0);
        so = tokio_stdin_stdout::stdout(0);
    }
    
    #[cfg(all(unix,not(feature="no_unix_stdio")))]
    {
        let stdin  = UnixFile::new_nb(std::io::stdin())?;
        let stdout = UnixFile::new_nb(std::io::stdout())?;
    
        si = stdin.into_reader(&handle)?;
        so = stdout.into_io(&handle)?;
        
        let ctrl_c = tokio_signal::ctrl_c(&handle).flatten_stream();
        let prog = ctrl_c.for_each(|()| {
            UnixFile::raw_new(std::io::stdin()).set_nonblocking(false);
            UnixFile::raw_new(std::io::stdout()).set_nonblocking(false);
            ::std::process::exit(0);
            Ok(())
        });
        handle.spawn(prog.map_err(|_|()));
    }
    Ok(Peer::new(si,so))
}

fn get_stdio_peer(handle: &Handle) -> BoxedNewPeerFuture {
    Box::new(futures::future::result(get_stdio_peer_impl(handle))) as BoxedNewPeerFuture
}

struct Transfer {
    from: Box<AsyncRead>,
    to:   Box<AsyncWrite>,
}
struct Session(Transfer,Transfer);

type WaitingForImplTraitFeature3 = futures::stream::StreamFuture<futures::sync::mpsc::Receiver<()>>;

impl Session {
    fn run(self, handle: &Handle) -> WaitingForImplTraitFeature3 {
        let (notif1,rcv) = mpsc::channel::<()>(0);
        let notif2 = notif1.clone();
        handle.spawn(
            my_copy::copy(self.0.from, self.0.to, true)
                .map_err(|_|())
                .map(|_|{notif1;()})
        );
        handle.spawn(
            my_copy::copy(self.1.from, self.1.to, true)
                .map_err(|_|())
                .map(|_|{notif2;()})
        );
        rcv.into_future()
    }
    fn new(peer1: Peer, peer2: Peer) -> Self {
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

fn run() -> Result<()> {
    let _        = std::env::args().nth(1).ok_or("Usage: websocat - ws[s]://...")?;
    let peeraddr = std::env::args().nth(2).ok_or("no second arg")?;

    //println!("Connecting to {}", peeraddr);
    let mut core = Core::new()?;
    let handle = core.handle();

    let h1 = core.handle();
    let h2 = core.handle();

    let runner = get_ws_client_peer(&h1, peeraddr.as_ref())
    .and_then(|ws_peer| {
        get_stdio_peer(&h2)
        .and_then(|std_peer| {
            let s = Session::new(ws_peer,std_peer);
            
            s.run(&handle)
                .map(|_|())
                .map_err(|_|unreachable!())
        })
    });

    core.run(runner)?;
    Ok(())
}

fn main() {
    let r = run();
    
    #[cfg(all(unix,not(feature="no_unix_stdio")))]
    {
        UnixFile::raw_new(std::io::stdin()).set_nonblocking(false);
        UnixFile::raw_new(std::io::stdout()).set_nonblocking(false);
    }
            
    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
