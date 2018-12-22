extern crate hyper;
extern crate websocket;

use self::websocket::client::async::ClientNew;
use self::websocket::stream::async::Stream as WsStream;
use self::websocket::ClientBuilder;
use futures::future::Future;
use futures::stream::Stream;

use std::cell::RefCell;
use std::rc::Rc;

use self::websocket::client::Url;

use super::{box_up_err, peer_err, peer_strerr, BoxedNewPeerFuture, Peer, Result};

use super::ws_peer::{Mode1, PeerForWs, WsReadWrapper, WsWriteWrapper};
use super::{once, ConstructParams, Options, PeerConstructor, Specifier};

use self::hyper::header::Headers;

#[derive(Debug, Clone)]
pub struct WsClient(pub Url);
impl Specifier for WsClient {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let url = self.0.clone();
        once(get_ws_client_peer(&url, p.program_options))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = WsClientClass,
    target = WsClient,
    prefixes = ["ws://"],
    arg_handling = {
        fn construct(self: &WsClientClass, arg: &str) -> super::Result<Rc<Specifier>> {
            Ok(Rc::new(WsClient(format!("ws:{}", arg).parse()?)))
        }
        fn construct_overlay(
            self: &WsClientClass,
            _inner: Rc<Specifier>,
        ) -> super::Result<Rc<Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
    },
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Insecure (ws://) WebSocket client. Argument is host and URL.

Example: connect to public WebSocket loopback and copy binary chunks from stdin to the websocket.

    websocat - ws://echo.websocket.org/
"#
);

#[cfg(feature = "ssl")]
#[derive(Debug, Clone)]
pub struct WsClientSecure(pub Url);
#[cfg(feature = "ssl")]
impl Specifier for WsClientSecure {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let url = self.0.clone();
        once(get_ws_client_peer(&url, p.program_options))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
#[cfg(feature = "ssl")]
specifier_class!(
    name = WsClientSecureClass,
    target = WsClientSecure,
    prefixes = ["wss://"],
    arg_handling = {
        fn construct(self: &WsClientSecureClass, arg: &str) -> super::Result<Rc<Specifier>> {
            Ok(Rc::new(WsClient(format!("wss:{}", arg).parse()?)))
        }
        fn construct_overlay(
            self: &WsClientSecureClass,
            _inner: Rc<Specifier>,
        ) -> super::Result<Rc<Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
    },
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Secure (wss://) WebSocket client. Argument is host and URL.

Example: forward TCP port 4554 to a websocket

    websocat tcp-l:127.0.0.1:4554 wss://127.0.0.1/some_websocket"#
);

#[derive(Debug)]
pub struct WsConnect<T: Specifier>(pub T);
impl<T: Specifier> Specifier for WsConnect<T> {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(p.clone());

        let url: Url = match p.program_options.ws_c_uri.parse() {
            Ok(x) => x,
            Err(e) => return PeerConstructor::ServeOnce(peer_err(e)),
        };

        let opts = p.program_options;

        inner.map(move |q, _| get_ws_client_peer_wrapped(&url, q, opts.clone()))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = WsConnectClass,
    target = WsConnect,
    prefixes = ["ws-c:", "c-ws:", "ws-connect:", "connect-ws:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Low-level WebSocket connector. Argument is a some another address. [A]

URL and Host: header being sent are independent from the underlying connection.

Example: connect to echo server in more explicit way

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:tcp:174.129.224.73:80

Example: connect to echo server, observing WebSocket TCP packet exchange

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:cmd:"socat -v -x - tcp:174.129.224.73:80"

"#
);

fn get_ws_client_peer_impl<S, F>(uri: &Url, opts: Rc<Options>, f: F) -> BoxedNewPeerFuture
where
    S: WsStream + Send + 'static,
    F: FnOnce(ClientBuilder) -> Result<ClientNew<S>>,
{
    let mode1 = if opts.websocket_text_mode {
        Mode1::Text
    } else {
        Mode1::Binary
    };

    let stage1 = ClientBuilder::from_url(uri);
    let stage2 = if opts.custom_headers.is_empty() {
        stage1
    } else {
        let mut h = Headers::new();
        for (hn, hv) in opts.custom_headers.clone() {
            h.append_raw(hn, hv);
        }
        stage1.custom_headers(&h)
    };
    let stage3 = if let Some(ref x) = opts.origin {
        stage2.origin(x.clone())
    } else {
        stage2
    };
    let stage4 = if let Some(ref p) = opts.websocket_protocol {
        stage3.add_protocol(p.to_owned())
    } else {
        stage3
    };
    let stage5 = if let Some(ref v) = opts.websocket_version {
        stage4.version(websocket::header::WebSocketVersion::Unknown(v.clone()))
    } else {
        stage4
    };
    let after_connect = match f(stage5) {
        Ok(x) => x,
        Err(_) => return peer_strerr("Failed to make TLS connector"),
    };
    Box::new(
        after_connect
            .map(move |(duplex, _)| {
                info!("Connected to ws",);
                let (sink, stream) = duplex.split();
                let mpsink = Rc::new(RefCell::new(sink));

                let ws_str = WsReadWrapper {
                    s: stream,
                    pingreply: mpsink.clone(),
                    debt: super::readdebt::ReadDebt(Default::default(), opts.read_debt_handling),
                };
                let ws_sin = WsWriteWrapper(mpsink, mode1, !opts.websocket_dont_close);

                Peer::new(ws_str, ws_sin)
            }).map_err(box_up_err),
    ) as BoxedNewPeerFuture
}

pub fn get_ws_client_peer(uri: &Url, opts: Rc<Options>) -> BoxedNewPeerFuture {
    info!("get_ws_client_peer");

    #[allow(unused)]
    let tls_insecure = opts.tls_insecure;
    get_ws_client_peer_impl(uri, opts, |before_connect| {
        #[cfg(feature = "ssl")]
        let after_connect = {
            let mut tls_opts = None;
            if tls_insecure {
                tls_opts = Some(
                    super::ssl_peer::native_tls::TlsConnector::builder()
                        .danger_accept_invalid_certs(true)
                        .danger_accept_invalid_hostnames(true)
                        .build()?
                );
            };
            before_connect.async_connect(tls_opts)
        };
        #[cfg(not(feature = "ssl"))]
        let after_connect = before_connect.async_connect_insecure();
        Ok(after_connect)
    })
}

unsafe impl Send for PeerForWs {
    //! https://github.com/cyderize/rust-websocket/issues/168
}

pub fn get_ws_client_peer_wrapped(uri: &Url, inner: Peer, opts: Rc<Options>) -> BoxedNewPeerFuture {
    info!("get_ws_client_peer_wrapped");
    get_ws_client_peer_impl(uri, opts, |before_connect| {
        Ok(before_connect.async_connect_on(PeerForWs(inner)))
    })
}
