#![allow(unused)]
extern crate websocket;

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use self::websocket::{ClientBuilder, OwnedMessage, WebSocketError};
use tokio_io::{self,AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

use std::rc::Rc;
use std::cell::RefCell;

use self::websocket::server::upgrade::async::IntoWs;

use futures::Async::{Ready, NotReady};

use super::{Peer, io_other_error, brokenpipe, wouldblock, BoxedNewPeerFuture};

struct PeerForWs(Peer);

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

pub fn ws_upgrade_peer(inner_peer : Peer) -> BoxedNewPeerFuture {
    let step1 = PeerForWs(inner_peer);
    let step2 : Box<Future<Item=self::websocket::server::upgrade::async::Upgrade<_>,Error=_>> = step1.into_ws();
    let step3 = step2
        .map_err(|(_,_,_,e)| WebSocketError::RequestError("something wrong 1") )
        .and_then(|x| {
            x.accept().map(|(y,_)| {
                let (sink, stream) = y.split();
                let mpsink = Rc::new(RefCell::new(sink));
            
                let ws_str = super::ws_peer::WsReadWrapper {
                    s: stream,
                    pingreply: mpsink.clone(),
                    debt: None,
                };
                let ws_sin = super::ws_peer::WsWriteWrapper(mpsink);
                
                let ws = Peer::new(ws_str, ws_sin);
                ws
            })
        });
    let step4 = step3.map_err(|e| Box::new(e) as Box<std::error::Error>);
    Box::new(step4) as BoxedNewPeerFuture
}

