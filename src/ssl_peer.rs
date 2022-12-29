use futures::future::{ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_err, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, Options, PeerConstructor, Specifier};

pub extern crate native_tls;
extern crate readwrite;
extern crate tokio_tls;

use self::native_tls::{Identity as Pkcs12, TlsAcceptor, TlsConnector};
use self::tokio_tls::{TlsAcceptor as TlsAcceptorExt, TlsConnector as TlsConnectorExt};

use std::ffi::{OsStr, OsString};

pub fn interpret_pkcs12(x: &OsStr) -> ::std::result::Result<Vec<u8>, OsString> {
    match (|| {
        use std::io::Read;
        let mut f = ::std::fs::File::open(x)?;
        let mut v = Vec::with_capacity(2048);
        f.read_to_end(&mut v)?;
        Ok(v)
    })() {
        Err(e) => {
            let e: Box<dyn (::std::error::Error)> = e;
            let o: OsString = format!("{}", e).into();
            Err(o)
        }
        Ok(x) => Ok(x),
    }
}

#[derive(Debug)]
pub struct TlsConnect<T: Specifier>(pub T);
impl<T: Specifier> Specifier for TlsConnect<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            ssl_connect(
                p,
                l2r,
                cp.program_options.tls_domain.clone(),
                cp.program_options.tls_insecure,
                cp.program_options.client_pkcs12_der.clone(),
                cp.program_options.client_pkcs12_passwd.clone(),
            )
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = TlsConnectClass,
    target = TlsConnect,
    prefixes = ["ssl-connect:","ssl-c:","ssl:","tls:","tls-connect:","tls-c:","c-ssl:","connect-ssl:","c-tls:","connect-tls:"],
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

#[derive(Debug)]
pub struct TlsAccept<T: Specifier>(pub T);
impl<T: Specifier> Specifier for TlsAccept<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| ssl_accept(p, l2r, cp.program_options.clone()))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = TlsAcceptClass,
    target = TlsAccept,
    prefixes = [
        "ssl-accept:",
        "ssl-a:",
        "tls-a:",
        "tls-accept:",
        "a-ssl:",
        "accept-ssl:",
        "accept-tls:",
        "accept-tls:"
    ],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Accept an TLS connection using arbitrary backing stream. [A]

Example: The same as in TlsListenClass's example, but with manual acceptor

    websocat -E -b --pkcs12-der=q.pkcs12 tls-a:tcp-l:127.0.0.1:1234 mirror:
"#
);

specifier_alias!(
    name = TlsListenClass,
    prefixes = [
        "ssl-listen:",
        "ssl-l:",
        "tls-l:",
        "tls-listen:",
        "l-ssl:",
        "listen-ssl:",
        "listen-tls:",
        "listen-tls:"
    ],
    alias = "tls-accept:tcp-l:",
    help = r#"
Listen for SSL connections on a TCP port

Example: Non-websocket SSL echo server

    websocat -E -b --pkcs12-der=q.pkcs12 ssl-listen:127.0.0.1:1234 mirror:
    socat - ssl:127.0.0.1:1234,verify=0
"#
);

specifier_alias!(
    name = WssListenClass,
    prefixes = ["wss-listen:", "wss-l:", "l-wss:", "wss-listen:"],
    alias = "ws-u:tls-accept:tcp-l:",
    help = r#"
Listen for secure WebSocket connections on a TCP port

Example: wss:// echo server + client for testing

    websocat -E -t --pkcs12-der=q.pkcs12 wss-listen:127.0.0.1:1234 mirror:
    websocat --ws-c-uri=wss://localhost/ -t - ws-c:cmd:'socat - ssl:127.0.0.1:1234,verify=0'

See [moreexamples.md](./moreexamples.md) for info about generation of `q.pkcs12`.
"#
);

use tokio_io::AsyncRead;

pub fn ssl_connect(
    inner_peer: Peer,
    _l2r: L2rUser,
    dom: Option<String>,
    tls_insecure: bool,
    client_identity : Option<Vec<u8>>,
    client_identity_password : Option<String>,
) -> BoxedNewPeerFuture {
    let hup = inner_peer.2;
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);

    fn gettlsc(nohost: bool, noverify: bool, client_identity : Option<Vec<u8>>, client_identity_password : Option<String>) -> native_tls::Result<TlsConnectorExt> {
        let mut b = TlsConnector::builder();
        if nohost {
            b.danger_accept_invalid_hostnames(true);
        }
        if noverify {
            b.danger_accept_invalid_hostnames(true);
            b.danger_accept_invalid_certs(true);
        }
        
        if let Some(client_ident) = client_identity {
            let identity = super::ssl_peer::native_tls::Identity::from_pkcs12(
                &client_ident,
                &client_identity_password.unwrap_or("".to_string()),
            )
            .map_err(|e| {
                error!(
                    "Unable to parse client identity: {}\nContinuing without a client identity",
                    e
                )
            })
            .ok();
            if let Some(x) = identity {
                b.identity(x);
            }
        }

        let tlsc: TlsConnector = b.build()?;
        Ok(TlsConnectorExt::from(tlsc))
    }

    let tls = match gettlsc(dom.is_none(), tls_insecure, client_identity, client_identity_password) {
        Ok(x) => x,
        Err(e) => return peer_err(e),
    };

    info!("Connecting to TLS");
    if let Some(dom) = dom {
        Box::new(
            tls.connect(dom.as_str(), squashed_peer)
                .map_err(box_up_err)
                .and_then(move |tls_stream| {
                    info!("Connected to TLS");
                    let (r, w) = tls_stream.split();
                    ok(Peer::new(r, w, hup))
                }),
        )
    } else {
        Box::new(tls.connect("domainverificationdisabled", squashed_peer).map_err(box_up_err).and_then(move |tls_stream| {
            warn!("Connected to TLS without proper verification of certificate. Use --tls-domain option.");
            let (r,w) = tls_stream.split();
            ok(Peer::new(r,w, hup))
        }))
    }
}

pub fn ssl_accept(inner_peer: Peer, _l2r: L2rUser, progopt: Rc<Options>) -> BoxedNewPeerFuture {
    let hup = inner_peer.2;
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);

    fn gettlsa(cert: &[u8], passwd: &str) -> native_tls::Result<TlsAcceptorExt> {
        let pkcs12 = Pkcs12::from_pkcs12(&cert[..], passwd)?;
        Ok(TlsAcceptorExt::from(TlsAcceptor::builder(pkcs12).build()?))
    }

    let der = progopt
        .pkcs12_der
        .as_ref()
        .expect("lint should have caught the missing pkcs12_der option");
    let passwd = progopt
        .pkcs12_passwd
        .as_ref()
        .map(|x| x.as_str())
        .unwrap_or("");
    let tls = match gettlsa(der, passwd) {
        Ok(x) => x,
        Err(e) => return peer_err(e),
    };

    debug!("Accepting a TLS connection");
    Box::new(
        tls.accept(squashed_peer)
            .map_err(box_up_err)
            .and_then(move |tls_stream| {
                info!("Accepted TLS connection");
                match tls_stream.get_ref().peer_certificate() {
                    Ok(Some(_cert)) => {
                        // Does not actually work with native-tls
                        info!("  the client presented an identity certificate.");
                    }
                    Ok(None) => {
                        debug!("  no identity certificate from the client. But Websocat may have failed to request it.");
                    }
                    Err(e) => {
                        warn!("Error getting identity certificate from client: {}", e);
                    }
                }
                let (r, w) = tls_stream.split();
                ok(Peer::new(r, w, hup))
            }),
    )
}
