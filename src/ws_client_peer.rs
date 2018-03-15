extern crate websocket;

use tokio_core::reactor::{Handle};
use futures::future::Future;
use futures::stream::Stream;
use self::websocket::{ClientBuilder,client::async::ClientNew};
use self::websocket::stream::async::{Stream as WsStream};

use std::rc::Rc;
use std::cell::RefCell;

use self::websocket::client::Url;

use super::{Peer, BoxedNewPeerFuture, box_up_err};

use super::ws_peer::{WsReadWrapper, WsWriteWrapper, PeerForWs};

fn get_ws_client_peer_impl<S,F>(uri: &Url, f: F) -> BoxedNewPeerFuture 
    where S:WsStream+Send+'static, F : FnOnce(ClientBuilder)->ClientNew<S>
{
    let stage1 = ClientBuilder::from_url(uri);
    let before_connect = stage1
        .add_protocol("rust-websocket");
    let after_connect = f(before_connect);
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

pub fn get_ws_client_peer(handle: &Handle, uri: &Url) -> BoxedNewPeerFuture {
    get_ws_client_peer_impl(uri, |before_connect| {
        #[cfg(feature="ssl")]
        let after_connect = before_connect
            .async_connect(None, handle);
        #[cfg(not(feature="ssl"))]
        let after_connect = before_connect
            .async_connect_insecure(handle);
        after_connect
    })
}

unsafe impl Send for PeerForWs {
    //! https://github.com/cyderize/rust-websocket/issues/168
}

pub fn get_ws_client_peer_wrapped(uri: &Url, inner: Peer) -> BoxedNewPeerFuture {
    get_ws_client_peer_impl(uri, |before_connect| {
        let after_connect = before_connect
            .async_connect_on(PeerForWs(inner));
        after_connect
    })
}
