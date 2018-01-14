#![allow(unused)]

extern crate websocket;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_stdin_stdout;

use std::thread;
use std::io::stdin;
use tokio_core::reactor::Core;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use websocket::stream::async::Stream as WsStream;
use futures::Async::{Ready, NotReady};

use tokio_io::io::copy;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

type WaitingForImplTraitFeature0 = tokio_io::codec::Framed<std::boxed::Box<websocket::async::Stream + std::marker::Send>, websocket::async::MessageCodec<websocket::OwnedMessage>>;
type WaitingForImplTraitFeature1 = futures::stream::SplitStream<WaitingForImplTraitFeature0>;
type WaitingForImplTraitFeature2 = futures::stream::SplitSink<WaitingForImplTraitFeature0>;

struct WsReadWrapper(WaitingForImplTraitFeature1);

impl AsyncRead for WsReadWrapper {

}

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

impl Read for WsReadWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        match self.0.poll().map_err(io_other_error)? {
            Ready(Some(OwnedMessage::Close(_))) => {
                brokenpipe()
            },
            Ready(None) => {
                brokenpipe()
            }
            Ready(Some(OwnedMessage::Ping(_))) => {
                Ok(0)
                // TODO
            }
            Ready(Some(OwnedMessage::Pong(_))) => {
                Ok(0)
            }
            Ready(Some(OwnedMessage::Text(x))) => {
                let buf_in = x.as_str().as_bytes();
                let l = buf_in.len().min(buf.len());
                buf[..l].copy_from_slice(&buf_in[..l]);
                Ok(l)
                // TODO
            }
            Ready(Some(OwnedMessage::Binary(x))) => {
                let buf_in = x.as_slice();
                let l = buf_in.len().min(buf.len());
                buf[..l].copy_from_slice(&buf_in[..l]);
                Ok(l)
                // TODO
            }
            NotReady => {
                wouldblock()
            }
        }
    }
}

struct WsWriteWrapper(WaitingForImplTraitFeature2);

impl AsyncWrite for WsWriteWrapper {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        // TODO: check this
        Ok(Ready(()))
    }
}

impl Write for WsWriteWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = OwnedMessage::Binary(buf.to_vec());
        match self.0.start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => {
                wouldblock()
            },
            futures::AsyncSink::Ready => {
                Ok(buf.len())
            }
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self.0.poll_complete().map_err(io_other_error)? {
            NotReady => {
                wouldblock()
            },
            Ready(()) => {
                Ok(())
            }
        }
    }

}


fn run() -> Result<()> {
    let peeraddr = std::env::args().nth(1).ok_or("no arg")?;

    println!("Connecting to {}", peeraddr);
    let mut core = Core::new()?;
    let handle = core.handle();
    
    let si = tokio_stdin_stdout::stdin(0);
    let so = tokio_stdin_stdout::stdout(0);

    //let (usr_msg, stdin_ch) = mpsc::channel(0);
    
    let runner = ClientBuilder::new(peeraddr.as_ref())?
        .add_protocol("rust-websocket")
        .async_connect(None, &core.handle())
        .and_then(|(duplex, _)| {
            let (sink, stream) = duplex.split();
            
            let ws_str = WsReadWrapper(stream);
            let ws_sin = WsWriteWrapper(sink);
            
            handle.spawn(my_copy::copy(si, ws_sin).map(|_|()).map_err(|_|()));
            my_copy::copy(ws_str, so).map_err(|e| WebSocketError::IoError(e))
            
            /*stream.filter_map(|message| {
                                  println!("Received Message: {:?}", message);
                                  match message {
                                      OwnedMessage::Close(e) => Some(OwnedMessage::Close(e)),
                                      OwnedMessage::Ping(d) => Some(OwnedMessage::Pong(d)),
                                      _ => None,
                                  }
                                 })
                  .select(stdin_ch.map_err(|_| WebSocketError::NoDataAvailable))
                  .forward(sink)
            */
        });
    core.run(runner)?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
