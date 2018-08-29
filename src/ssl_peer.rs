#![allow(unused)]
use futures::future::{err, ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_strerr, BoxedNewPeerFuture, Peer, Result};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use tokio_io::io::{read_exact, write_all};

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

extern crate tokio_tls;
extern crate native_tls;
extern crate readwrite;

use self::tokio_tls::TlsConnectorExt;
use self::native_tls::TlsConnector;

#[derive(Debug)]
pub struct TlsConnect<T: Specifier>(pub T);
impl<T: Specifier> Specifier for TlsConnect<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            ssl_connect(p, l2r, cp.program_options.tls_domain.clone())
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = TlsConnectClass,
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

use tokio_io::AsyncRead;

pub fn ssl_connect(
    inner_peer: Peer,
    _l2r: L2rUser,
    dom: Option<String>,
) -> BoxedNewPeerFuture {
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);
    
    fn gettlsc() -> native_tls::Result<TlsConnector> {
        Ok(TlsConnector::builder()?.build()?)
    }
    let tls = if let Ok(x) = gettlsc() {
        x
    } else {
        return peer_strerr("Failed to initialize TLS");
    };
    
    if let Some(dom) = dom {
        Box::new(tls.connect_async(dom.as_str(), squashed_peer).map_err(box_up_err).and_then(move |tls_stream| {
            info!("Connected to TLS");
            let (r,w) = tls_stream.split();
            ok(Peer::new(r,w))
        }))
    } else {
        Box::new(tls.danger_connect_async_without_providing_domain_for_certificate_verification_and_server_name_indication(squashed_peer).map_err(box_up_err).and_then(move |tls_stream| {
            warn!("Connected to TLS without proper verification of certificate. Use --tls-domain option.");
            let (r,w) = tls_stream.split();
            ok(Peer::new(r,w))
        }))
    }
}
