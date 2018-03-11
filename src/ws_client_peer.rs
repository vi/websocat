extern crate websocket;

use tokio_core::reactor::{Handle};
use futures::future::Future;
use futures::stream::Stream;
use self::websocket::{ClientBuilder};

use std::rc::Rc;
use std::cell::RefCell;

use super::{Peer, BoxedNewPeerFuture, box_up_err, peer_err};

use super::ws_peer::{WsReadWrapper, WsWriteWrapper};

pub fn get_ws_client_peer(handle: &Handle, uri: &str) -> BoxedNewPeerFuture {
    let stage1 = match ClientBuilder::new(uri) {
        Ok(x) => x,
        Err(e) => return peer_err(e),
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
        .map_err(box_up_err)
    ) as BoxedNewPeerFuture
}

