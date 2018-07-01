extern crate websocket;

use self::websocket::WebSocketError;
use futures::future::Future;
use futures::stream::Stream;

use std::cell::RefCell;
use std::rc::Rc;

use self::websocket::server::upgrade::async::IntoWs;

use super::ws_peer::{Mode1, PeerForWs, WsReadWrapper, WsWriteWrapper};
use super::{box_up_err, io_other_error, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

#[derive(Debug)]
pub struct WsServer<T: Specifier>(pub T);
impl<T: Specifier> Specifier for WsServer<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let mode1 = if cp.program_options.websocket_text_mode {
            Mode1::Text
        } else {
            Mode1::Binary
        };
        let inner = self.0.construct(cp.clone());
        inner.map(move |p| ws_upgrade_peer(p, mode1))
    }
    specifier_boilerplate!(typ=WebSocket noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = WsServerClass,
    target = WsServer,
    prefixes = ["ws-upgrade:", "upgrade-ws:", "ws-u:", "u-ws:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
WebSocket upgrader / raw server. Specify your own protocol instead of usual TCP.
or a subspecifier.

All other WebSocket server modes actually use this overlay under the hood.

Example: TODO
"#
);


specifier_alias!(
    name = WsTcpServerClass,
    prefixes = ["ws-listen:", "ws-l:", "l-ws:", "listen-ws:"],
    alias="ws-u:tcp-l:",
    help = r#"
WebSocket server. Argument is either IPv4 host and port to listen
or a subspecifier.

Example: Dump all incoming websocket data to console

    websocat ws-l:127.0.0.1:8808 -

Example: the same, but more verbose:

    websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-
"#
);


specifier_alias!(
    name = WsInetdServerClass,
    prefixes = ["inetd-ws:", "ws-inetd:"],
    alias="ws-u:inetd:",
    help = r#"
WebSocket inetd server.

TODO: transfer the example here
"#
);

specifier_alias!(
    name = WsUnixServerClass,
    prefixes = ["l-ws-unix:"],
    alias="ws-l:unix-l:",
    help = r#"
WebSocket UNIX socket-based server.
"#
);

specifier_alias!(
    name = WsAbstractUnixServerClass,
    prefixes = ["l-ws-abstract:"],
    alias="ws-l:abstract-l:",
    help = r#"
WebSocket abstract-namespaced UNIX socket server.
"#
);

/* 

     if x == "" {
                Err("Specify underlying protocol for ws-l:")?;
            }
            if let Some(c) = x.chars().next() {
                if c.is_numeric() || c == '[' {
                    // Assuming user uses old format like ws-l:127.0.0.1:8080
                    return spec(&("ws-l:tcp-l:".to_owned() + x));
                }
            }
            boxup(super::ws_server_peer::WsUpgrade(spec(x)?))
*/

pub fn ws_upgrade_peer(inner_peer: Peer, mode1: Mode1) -> BoxedNewPeerFuture {
    let step1 = PeerForWs(inner_peer);
    let step2: Box<
        Future<Item = self::websocket::server::upgrade::async::Upgrade<_>, Error = _>,
    > = step1.into_ws();
    let step3 = step2
        .map_err(|(_, _, _, e)| WebSocketError::IoError(io_other_error(e)))
        .and_then(move |x| {
            info!("Incoming connection to websocket: {}", x.request.subject.1);
            debug!("{:?}", x.request);
            debug!("{:?}", x.headers);
            x.accept().map(move |(y, headers)| {
                debug!("{:?}", headers);
                info!("Upgraded");
                let (sink, stream) = y.split();
                let mpsink = Rc::new(RefCell::new(sink));

                let ws_str = WsReadWrapper {
                    s: stream,
                    pingreply: mpsink.clone(),
                    debt: Default::default(),
                };
                let ws_sin = WsWriteWrapper(mpsink, mode1, true /* send Close on shutdown */);

                Peer::new(ws_str, ws_sin)
            })
        });
    let step4 = step3.map_err(box_up_err);
    Box::new(step4) as BoxedNewPeerFuture
}
