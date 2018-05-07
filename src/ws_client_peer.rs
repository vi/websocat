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

use super::ws_peer::{WsReadWrapper, WsWriteWrapper, PeerForWs, Mode1};
use super::{once,Specifier,ProgramState,PeerConstructor,Options};

#[derive(Debug,Clone)]
pub struct WsClient(pub Url);
impl Specifier for WsClient {
    fn construct(&self, h:&Handle, _: &mut ProgramState, opts: &Options) -> PeerConstructor {
        let mode1 = if opts.websocket_text_mode { Mode1::Text } else {Mode1::Binary};
        let url = self.0.clone();
        once(get_ws_client_peer(h, &url, mode1))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}

#[derive(Debug)]
pub struct WsConnect<T:Specifier>(pub T,pub Url);
impl<T:Specifier> Specifier for WsConnect<T> {
    fn construct(&self, h:&Handle, ps: &mut ProgramState, opts: &Options) -> PeerConstructor {
        let inner = self.0.construct(h, ps, opts);
        
        let url = self.1.clone();
        
        let mode1 = if opts.websocket_text_mode { Mode1::Text } else {Mode1::Binary};
        
        inner.map(move |q| {
            get_ws_client_peer_wrapped(&url, q, mode1)
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec typ=Other);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}



fn get_ws_client_peer_impl<S,F>(uri: &Url, mode1: Mode1, f: F) -> BoxedNewPeerFuture 
    where S:WsStream+Send+'static, F : FnOnce(ClientBuilder)->ClientNew<S>
{
    let stage1 = ClientBuilder::from_url(uri);
    let before_connect = stage1
        .add_protocol("rust-websocket"); // TODO: customizable protocol
    let after_connect = f(before_connect);
    Box::new(after_connect
        .map(move |(duplex, _)| {
            info!("Connected to ws",);
            let (sink, stream) = duplex.split();
            let mpsink = Rc::new(RefCell::new(sink));
            
            let ws_str = WsReadWrapper {
                s: stream,
                pingreply: mpsink.clone(),
                debt: Default::default(),
            };
            let ws_sin = WsWriteWrapper(mpsink, mode1);
            
            let ws = Peer::new(ws_str, ws_sin);
            ws
        })
        .map_err(box_up_err)
    ) as BoxedNewPeerFuture
}

pub fn get_ws_client_peer(handle: &Handle, uri: &Url, mode1: Mode1) -> BoxedNewPeerFuture {
    info!("get_ws_client_peer");
    get_ws_client_peer_impl(uri, mode1, |before_connect| {
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

pub fn get_ws_client_peer_wrapped(uri: &Url, inner: Peer, mode1: Mode1) -> BoxedNewPeerFuture {
    info!("get_ws_client_peer_wrapped");
    get_ws_client_peer_impl(uri, mode1, |before_connect| {
        let after_connect = before_connect
            .async_connect_on(PeerForWs(inner));
        after_connect
    })
}
