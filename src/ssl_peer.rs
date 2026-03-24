use futures::future::{ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_err, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, Options, PeerConstructor, Specifier};

pub extern crate native_tls;
pub extern crate openssl;
extern crate readwrite;
pub extern crate tokio_openssl;
extern crate tokio_tls;

use self::native_tls::{Identity as Pkcs12, TlsAcceptor, TlsConnector};
use self::tokio_tls::{TlsAcceptor as TlsAcceptorExt, TlsConnector as TlsConnectorExt};

use std::ffi::{OsStr, OsString};

pub fn get_keylog_path() -> Option<String> {
    std::env::var("SSLKEYLOGFILE")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Set up TLS key logging callback on an OpenSSL context builder.
/// Opens the file at `keylog_path` in append mode and installs a callback
/// that writes NSS key log format lines to it.
pub fn configure_keylog(ctx: &mut openssl::ssl::SslConnectorBuilder, keylog_path: &str) {
    match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(keylog_path)
    {
        Ok(file) => {
            let writer = std::sync::Mutex::new(file);
            ctx.set_keylog_callback(move |_ssl, line| {
                if let Ok(mut f) = writer.lock() {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", line);
                }
            });
            info!("TLS key logging enabled, writing to {}", keylog_path);
        }
        Err(e) => {
            warn!("Failed to open SSLKEYLOGFILE '{}': {}", keylog_path, e);
        }
    }
}

/// Same as configure_keylog but for SslAcceptorBuilder (which also derefs to SslContextBuilder).
fn configure_keylog_acceptor(ctx: &mut openssl::ssl::SslAcceptorBuilder, keylog_path: &str) {
    match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(keylog_path)
    {
        Ok(file) => {
            let writer = std::sync::Mutex::new(file);
            ctx.set_keylog_callback(move |_ssl, line| {
                if let Ok(mut f) = writer.lock() {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", line);
                }
            });
            info!("TLS key logging enabled, writing to {}", keylog_path);
        }
        Err(e) => {
            warn!("Failed to open SSLKEYLOGFILE '{}': {}", keylog_path, e);
        }
    }
}

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

fn ssl_connect_openssl(
    inner_peer: Peer,
    dom: Option<String>,
    tls_insecure: bool,
    client_identity: Option<Vec<u8>>,
    client_identity_password: Option<String>,
    keylog_path: String,
) -> BoxedNewPeerFuture {
    use self::openssl::ssl::{SslConnector as OpensslConnector, SslMethod, SslVerifyMode};
    use self::tokio_openssl::SslConnectorExt as OpensslConnectorExt;

    let hup = inner_peer.2;
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);

    let mut builder = match OpensslConnector::builder(SslMethod::tls()) {
        Ok(b) => b,
        Err(e) => return peer_err(e),
    };

    configure_keylog(&mut builder, &keylog_path);

    if tls_insecure || dom.is_none() {
        builder.set_verify(SslVerifyMode::NONE);
    }

    if let Some(pkcs12_der) = client_identity {
        match openssl::pkcs12::Pkcs12::from_der(&pkcs12_der) {
            Ok(pkcs12) => match pkcs12.parse2(&client_identity_password.unwrap_or_default()) {
                Ok(parsed) => {
                    if let Some(ref cert) = parsed.cert {
                        if let Err(e) = builder.set_certificate(cert) {
                            error!("Failed to set client certificate: {}", e);
                        }
                    }
                    if let Some(ref pkey) = parsed.pkey {
                        if let Err(e) = builder.set_private_key(pkey) {
                            error!("Failed to set client private key: {}", e);
                        }
                    }
                }
                Err(e) => error!("Failed to parse client PKCS12 identity: {}", e),
            },
            Err(e) => error!("Failed to decode client PKCS12 DER: {}", e),
        }
    }

    let connector = builder.build();
    let domain = dom.unwrap_or_else(|| "domainverificationdisabled".to_string());

    info!("Connecting to TLS (with key logging)");
    Box::new(
        connector
            .connect_async(&domain, squashed_peer)
            .map_err(|e| {
                Box::new(super::simple_err(format!(
                    "OpenSSL TLS handshake error: {}",
                    e
                ))) as Box<dyn std::error::Error>
            })
            .and_then(move |tls_stream| {
                info!("Connected to TLS (with key logging)");
                let (r, w) = tls_stream.split();
                ok(Peer::new(r, w, hup))
            }),
    )
}

pub fn ssl_connect(
    inner_peer: Peer,
    _l2r: L2rUser,
    dom: Option<String>,
    tls_insecure: bool,
    client_identity : Option<Vec<u8>>,
    client_identity_password : Option<String>,
) -> BoxedNewPeerFuture {
    if let Some(keylog_path) = get_keylog_path() {
        return ssl_connect_openssl(
            inner_peer,
            dom,
            tls_insecure,
            client_identity,
            client_identity_password,
            keylog_path,
        );
    }

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

fn ssl_accept_openssl(
    inner_peer: Peer,
    progopt: Rc<Options>,
    keylog_path: String,
) -> BoxedNewPeerFuture {
    use self::openssl::ssl::{SslAcceptor as OpensslAcceptor, SslMethod};
    use self::tokio_openssl::SslAcceptorExt as OpensslAcceptorExt;

    let hup = inner_peer.2;
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);

    let der = progopt
        .pkcs12_der
        .as_ref()
        .expect("lint should have caught the missing pkcs12_der option");
    let passwd = progopt.pkcs12_passwd.as_deref().unwrap_or("");

    let mut builder = match OpensslAcceptor::mozilla_intermediate(SslMethod::tls()) {
        Ok(b) => b,
        Err(e) => return peer_err(e),
    };

    configure_keylog_acceptor(&mut builder, &keylog_path);

    match openssl::pkcs12::Pkcs12::from_der(der) {
        Ok(pkcs12) => match pkcs12.parse2(passwd) {
            Ok(parsed) => {
                if let Some(ref cert) = parsed.cert {
                    if let Err(e) = builder.set_certificate(cert) {
                        error!("Failed to set server certificate: {}", e);
                        return peer_err(e);
                    }
                }
                if let Some(ref pkey) = parsed.pkey {
                    if let Err(e) = builder.set_private_key(pkey) {
                        error!("Failed to set server private key: {}", e);
                        return peer_err(e);
                    }
                }
                if let Some(ref ca_certs) = parsed.ca {
                    for ca_cert in ca_certs.iter() {
                        if let Err(e) = builder.add_extra_chain_cert(ca_cert.to_owned()) {
                            error!("Failed to add chain certificate: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse server PKCS12 identity: {}", e);
                return peer_err(e);
            }
        },
        Err(e) => {
            error!("Failed to decode server PKCS12 DER: {}", e);
            return peer_err(e);
        }
    }

    let acceptor = builder.build();

    debug!("Accepting a TLS connection (with key logging)");
    Box::new(
        acceptor
            .accept_async(squashed_peer)
            .map_err(|e| {
                Box::new(super::simple_err(format!(
                    "OpenSSL TLS accept error: {}",
                    e
                ))) as Box<dyn std::error::Error>
            })
            .and_then(move |tls_stream| {
                info!("Accepted TLS connection (with key logging)");
                let (r, w) = tls_stream.split();
                ok(Peer::new(r, w, hup))
            }),
    )
}

pub fn ssl_accept(inner_peer: Peer, _l2r: L2rUser, progopt: Rc<Options>) -> BoxedNewPeerFuture {
    if let Some(keylog_path) = get_keylog_path() {
        return ssl_accept_openssl(inner_peer, progopt, keylog_path);
    }

    let hup = inner_peer.2;
    let squashed_peer = readwrite::ReadWriteAsync::new(inner_peer.0, inner_peer.1);

    fn gettlsa(cert: &[u8], passwd: &str) -> native_tls::Result<TlsAcceptorExt> {
        let pkcs12 = Pkcs12::from_pkcs12(cert, passwd)?;
        Ok(TlsAcceptorExt::from(TlsAcceptor::builder(pkcs12).build()?))
    }

    let der = progopt
        .pkcs12_der
        .as_ref()
        .expect("lint should have caught the missing pkcs12_der option");
    let passwd = progopt
        .pkcs12_passwd.as_deref()
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
