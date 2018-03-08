extern crate websocket;

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use self::websocket::{ClientBuilder, OwnedMessage};
use tokio_io::{self,AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use futures::Async::{Ready, NotReady};

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture};

#[cfg(feature="ssl")]
type WaitingForImplTraitFeature0 = tokio_io::codec::Framed<std::boxed::Box<websocket::async::Stream + std::marker::Send>, websocket::async::MessageCodec<websocket::OwnedMessage>>;
#[cfg(not(feature="ssl"))]
type WaitingForImplTraitFeature0 = tokio_io::codec::Framed<websocket::async::TcpStream, websocket::async::MessageCodec<websocket::OwnedMessage>>;
type WaitingForImplTraitFeature2 = futures::stream::SplitSink<WaitingForImplTraitFeature0>;
type WsSource = futures::stream::SplitStream<WaitingForImplTraitFeature0>;
type MultiProducerWsSink = Rc<RefCell<WaitingForImplTraitFeature2>>;


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

pub fn get_ws_client_peer(handle: &Handle, uri: &str) -> BoxedNewPeerFuture {
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
        let _ = sink.start_send(OwnedMessage::Close(None))
            .map_err(|_|())
            .map(|_|());
        let _ = sink.poll_complete()
            .map_err(|_|())
            .map(|_|());
    }
}

