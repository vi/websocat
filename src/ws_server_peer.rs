extern crate hyper;
extern crate websocket;

use self::hyper::uri::RequestUri::AbsolutePath;

use self::websocket::WebSocketError;
use futures::future::{err, Future};
use futures::stream::Stream;

use std::cell::RefCell;
use std::rc::Rc;

use options::StaticFile;

use self::websocket::server::upgrade::async::IntoWs;

use super::readdebt::{DebtHandling, ReadDebt};
use super::ws_peer::{Mode1, PeerForWs, WsReadWrapper, WsWriteWrapper};
use super::{box_up_err, io_other_error, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};

#[derive(Debug)]
pub struct WsServer<T: Specifier>(pub T);
impl<T: Specifier> Specifier for WsServer<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let mode1 = if cp.program_options.websocket_text_mode {
            Mode1::Text
        } else {
            Mode1::Binary
        };
        let restrict_uri = Rc::new(cp.program_options.restrict_uri.clone());
        let serve_static_files = Rc::new(cp.program_options.serve_static_files.clone());
        let inner = self.0.construct(cp.clone());
        //let l2r = cp.left_to_right;
        let rdh = cp.program_options.read_debt_handling;
        inner.map(move |p, l2r| {
            ws_upgrade_peer(
                p,
                mode1,
                rdh,
                restrict_uri.clone(),
                serve_static_files.clone(),
                cp.program_options.ws_ping_interval,
                cp.program_options.ws_ping_timeout,
                cp.program_options.websocket_reply_protocol.clone(),
                cp.program_options.custom_reply_headers.clone(),
                l2r,
            )
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
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
WebSocket upgrader / raw server. Specify your own protocol instead of usual TCP. [A]

All other WebSocket server modes actually use this overlay under the hood.

Example: serve incoming connection from socat

    socat tcp-l:1234,fork,reuseaddr exec:'websocat -t ws-u\:stdio\: mirror\:'
"#
);

specifier_alias!(
    name = WsTcpServerClass,
    prefixes = ["ws-listen:", "ws-l:", "l-ws:", "listen-ws:"],
    alias = "ws-u:tcp-l:",
    help = r#"
WebSocket server. Argument is host and port to listen.

Example: Dump all incoming websocket data to console

    websocat ws-l:127.0.0.1:8808 -

Example: the same, but more verbose:

    websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-
"#
);

specifier_alias!(
    name = WsInetdServerClass,
    prefixes = ["inetd-ws:", "ws-inetd:"],
    alias = "ws-u:inetd:",
    help = r#"
WebSocket inetd server. [A]

TODO: transfer the example here
"#
);

specifier_alias!(
    name = WsUnixServerClass,
    prefixes = ["l-ws-unix:"],
    alias = "ws-u:unix-l:",
    help = r#"
WebSocket UNIX socket-based server. [A]
"#
);

specifier_alias!(
    name = WsAbstractUnixServerClass,
    prefixes = ["l-ws-abstract:"],
    alias = "ws-l:abstract-l:",
    help = r#"
WebSocket abstract-namespaced UNIX socket server. [A]
"#
);

#[path = "http_serve.rs"]
pub mod http_serve;

pub fn ws_upgrade_peer(
    inner_peer: Peer,
    mode1: Mode1,
    ws_read_debt_handling: DebtHandling,
    restrict_uri: Rc<Option<String>>,
    serve_static_files: Rc<Vec<StaticFile>>,
    ping_interval: Option<u64>,
    ping_timeout: Option<u64>,
    websocket_protocol: Option<String>,
    custom_reply_headers: Vec<(String, Vec<u8>)>,
    l2r: L2rUser,
) -> BoxedNewPeerFuture {
    let step1 = PeerForWs(inner_peer);
    let step2: Box<
        dyn Future<Item = self::websocket::server::upgrade::async::Upgrade<_>, Error = _>,
    > = step1.into_ws();
    let step3 = step2
        .or_else(|(innerpeer, hyper_incoming, _bytesmut, e)| {
            http_serve::http_serve(innerpeer.0, hyper_incoming, serve_static_files)
            .then(|_|
                err(WebSocketError::IoError(io_other_error(e)))
            )
        })
        .and_then(
            move |mut x| -> Box<dyn Future<Item = Peer, Error = websocket::WebSocketError>> {
                info!("Incoming connection to websocket: {}", x.request.subject.1);

                use ::websocket::header::WebSocketProtocol;

                let mut protocol_check = true;
                {
                    let pp : Option<&WebSocketProtocol> = x.request.headers.get();
                    if let Some(rp) = websocket_protocol {
                        // Unconditionally set this protocol
                        x.headers.set_raw("Sec-WebSocket-Protocol",
                            vec![rp.as_bytes().to_vec()],
                        );
                        // Warn if not present in client protocols
                        let mut present = false;
                        if let Some(pp) = pp {
                            if let Some(pp) = pp.iter().next() {
                                if pp == &rp {
                                    present = true;
                                }
                            }
                        }
                        if !present {
                            if pp.is_none() {
                                warn!("Client failed to specify Sec-WebSocket-Protocol header. Replying with it anyway, against the RFC.");
                            } else {
                                protocol_check = false;
                            }
                        }
                    } else {
                        // No protocol specified, just choosing the first if any.
                        if let Some(pp) = pp {
                            if pp.len() > 1 {
                                warn!("Multiple `Sec-WebSocket-Protocol`s specified in the request. Choosing the first one. Use --server-protocol to make it explicit.")
                            }
                            if let Some(pp) = pp.iter().next() {
                                x.headers.set_raw(
                                    "Sec-WebSocket-Protocol",
                                    vec![pp.as_bytes().to_vec()],
                                );
                            }
                        }
                    }
                }

                for (hn, hv) in custom_reply_headers {
                    x.headers.append_raw(hn, hv);
                }

                debug!("{:?}", x.request);
                debug!("{:?}", x.headers);

                if !protocol_check {
                    return Box::new(
                            x.reject()
                                .and_then(|_| {
                                    warn!("Requested Sec-WebSocket-Protocol does not match --server-protocol option");
                                    ::futures::future::err(::util::simple_err(
                                        "Requested Sec-WebSocket-Protocol does not match --server-protocol option"
                                            .to_string(),
                                    ))
                                })
                                .map_err(|e| websocket::WebSocketError::IoError(io_other_error(e))),
                        )
                            as Box<dyn Future<Item = Peer, Error = websocket::WebSocketError>>;
                }
                
                
                match l2r {
                    L2rUser::FillIn(ref y) => {
                        let uri = &x.request.subject.1;
                        let mut z = y.borrow_mut();
                        z.uri = Some(format!("{}", uri));
                    },
                    L2rUser::ReadFrom(_) => {},
                }
                
                
                if let Some(ref restrict_uri) = *restrict_uri {
                    let check_passed = match x.request.subject.1 {
                        AbsolutePath(ref x) if x == restrict_uri => true,
                        _ => false,
                    };
                    if !check_passed {
                        return Box::new(
                            x.reject()
                                .and_then(|_| {
                                    warn!("Incoming request URI doesn't match the --restrict-uri value");
                                    ::futures::future::err(::util::simple_err(
                                        "Request URI doesn't match --restrict-uri parameter"
                                            .to_string(),
                                    ))
                                })
                                .map_err(|e| websocket::WebSocketError::IoError(io_other_error(e))),
                        )
                            as Box<dyn Future<Item = Peer, Error = websocket::WebSocketError>>;
                    }
                };
                Box::new(x.accept().map(move |(y, headers)| {
                    debug!("{:?}", headers);
                    info!("Upgraded");
                    let (sink, stream) = y.split();
                    let mpsink = Rc::new(RefCell::new(sink));

                    // FIXME: ping code duplicate between client and server

                    let ping_aborter = if let Some(d) = ping_interval {
                        debug!("Starting pinger");

                        let (tx,rx) = ::futures::unsync::oneshot::channel();

                        let intv = ::std::time::Duration::from_secs(d);
                        let pinger = super::ws_peer::WsPinger::new(
                            mpsink.clone(),
                            intv,
                            rx,
                        );
                        ::tokio_current_thread::spawn(pinger);
                        Some(tx)
                    } else {
                        None
                    };

                    let pong_timeout = if let Some(d) = ping_timeout {
                        let to = ::std::time::Duration::from_secs(d);
                        let de = ::tokio_timer::Delay::new(std::time::Instant::now() + to);
                        Some((de, to))
                    } else {
                        None
                    };

                    let ws_str = WsReadWrapper {
                        s: stream,
                        pingreply: mpsink.clone(),
                        debt: ReadDebt(Default::default(), ws_read_debt_handling),
                        pong_timeout,
                        ping_aborter,
                    };
                    let ws_sin =
                        WsWriteWrapper(mpsink, mode1, true /* send Close on shutdown */);

                    Peer::new(ws_str, ws_sin)
                })) as Box<dyn Future<Item = Peer, Error = websocket::WebSocketError>>
            },
        );
    let step4 = step3.map_err(box_up_err);
    Box::new(step4) as BoxedNewPeerFuture
}
