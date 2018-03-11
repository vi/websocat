extern crate websocket;

use std;
use futures;
use futures::sink::Sink;
use futures::stream::Stream;
use self::websocket::{OwnedMessage};
use self::websocket::stream::async::{Stream as WsStream};
use tokio_io::{self,AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use futures::Async::{Ready, NotReady};

use super::{io_other_error, brokenpipe, wouldblock, Peer};

type MultiProducerWsSink<T> = Rc<RefCell<futures::stream::SplitSink<tokio_io::codec::Framed<T, websocket::async::MessageCodec<websocket::OwnedMessage>>>>>;
type WsSource<T> = futures::stream::SplitStream<tokio_io::codec::Framed<T, websocket::async::MessageCodec<websocket::OwnedMessage>>>;

pub struct WsReadWrapper<T:WsStream+'static> {
    pub s: WsSource<T>,
    pub pingreply : MultiProducerWsSink<T>,
    pub debt: Option<Vec<u8>>,
}

impl<T:WsStream+'static>  AsyncRead for WsReadWrapper<T>
{}

impl<T:WsStream+'static>  WsReadWrapper<T>  {
    fn process_message(&mut self, buf: &mut [u8], buf_in: &[u8]) -> std::result::Result<usize, std::io::Error> {
        let l = buf_in.len().min(buf.len());
        buf[..l].copy_from_slice(&buf_in[..l]);
        
        if l < buf_in.len() {
            self.debt = Some(buf_in[l..].to_vec());
        }
        
        Ok(l)
    }
}

impl<T:WsStream+'static>  Read for WsReadWrapper<T>
{
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

pub struct WsWriteWrapper<T:WsStream+'static>(pub MultiProducerWsSink<T>);

impl<T:WsStream+'static> AsyncWrite for WsWriteWrapper<T> {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        // TODO: check this
        Ok(Ready(()))
    }
}

impl<T:WsStream+'static> Write for WsWriteWrapper<T> {
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

impl<T:WsStream+'static> Drop for WsWriteWrapper<T> {
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


pub struct PeerForWs(pub Peer);

//implicit impl websocket::stream::async::Stream for PeerForWs {}

impl AsyncRead for PeerForWs{}
impl Read for PeerForWs {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        (self.0).0.read(buf)
    }
}
impl AsyncWrite for PeerForWs{
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        (self.0).1.shutdown()
    }
}
impl Write for PeerForWs {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (self.0).1.write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        (self.0).1.flush()
    }
}
