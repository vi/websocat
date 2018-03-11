extern crate websocket;

use futures::future::Future;
use futures::stream::Stream;
use self::websocket::{WebSocketError};

use std::rc::Rc;
use std::cell::RefCell;

use self::websocket::server::upgrade::async::IntoWs;

use super::{Peer, io_other_error, BoxedNewPeerFuture, box_up_err};
use super::ws_peer::{WsReadWrapper, WsWriteWrapper, PeerForWs};

pub fn ws_upgrade_peer(inner_peer : Peer) -> BoxedNewPeerFuture {
    let step1 = PeerForWs(inner_peer);
    let step2 : Box<Future<Item=self::websocket::server::upgrade::async::Upgrade<_>,Error=_>> = step1.into_ws();
    let step3 = step2
        .map_err(|(_,_,_,e)| WebSocketError::IoError(io_other_error(e)) )
        .and_then(|x| {
            x.accept().map(|(y,_)| {
                let (sink, stream) = y.split();
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
        });
    let step4 = step3.map_err(box_up_err);
    Box::new(step4) as BoxedNewPeerFuture
}

