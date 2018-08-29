#![allow(unused)]
use futures::future::{err, ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_strerr, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use tokio_io::io::{read_exact, write_all};

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};


#[derive(Debug)]
pub struct TlsConnect<T: Specifier>(pub T);
impl<T: Specifier> Specifier for TlsConnect<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            ssl_connect(p, l2r)
        })
    }
    specifier_boilerplate!(typ=WebSocket noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = TclConnectClass,
    target = TlsConnect,
    prefixes = ["ssl-c","ssl:","tls:","ssl-connect:","tls-connect:","c-ssl:","connect-ssl","c-tls:","connect-tls:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Overlay to add TLS encryption atop of existing connection [A]

Example: manually connect to a secure websocket

    websocat -t - ws-c:tls-c:tcp:174.129.224.73:1080 --ws-c-uri ws://echo.websocket.org --tls-domain echo.websocket.org

For a user-friendly solution, see --socks5 command-line option
"#
);


pub fn ssl_connect(
    inner_peer: Peer,
    _l2r: L2rUser,
) -> BoxedNewPeerFuture {
    Box::new(ok(inner_peer))
}
