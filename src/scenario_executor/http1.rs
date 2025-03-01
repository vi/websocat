use anyhow::bail;
use base64::Engine as _;
use bytes::{Bytes, BytesMut};
use futures::FutureExt;
use http::{header, Response, StatusCode};
use hyper::client::conn::http1::{Connection, SendRequest};
use hyper_util::rt::TokioIo;
use rand::{Rng, SeedableRng};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use sha1::{Digest, Sha1};
use std::pin::Pin;
use tokio::io::AsyncWrite;
use tracing::{debug, debug_span, error, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, Hangup, StreamRead, StreamSocket, StreamWrite, Task},
    utils1::{HandleExt, HandleExt2, RhResult, SimpleErr, TaskHandleExt2},
};

type EmptyBody = http_body_util::Empty<bytes::Bytes>;
type IoType = TokioIo<tokio::io::Join<StreamRead, Pin<Box<dyn AsyncWrite + Send>>>>;
pub type IncomingRequest = hyper::Request<hyper::body::Incoming>;
pub type OutgoingResponse = Response<EmptyBody>;
pub type IncomingResponse = Response<hyper::body::Incoming>;
pub type OutgoingRequest = hyper::Request<EmptyBody>;
pub struct Http1Client {
    sr: SendRequest<EmptyBody>,
    hup: Option<Hangup>,
}

static MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

//@ Converts a downstream stream socket into a HTTP 1 client, suitable for sending e.g. WebSocket upgrade request.
fn http1_client(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
) -> RhResult<Handle<Http1Client>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "http1_client");
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Http1ClientOpts {}
    let _opts: Http1ClientOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "options parsed");

    let Some(StreamSocket {
        read: Some(r),
        write: Some(w),
        close: c,
    }) = inner.lut()
    else {
        return Err(ctx.err("Incomplete underlying socket specified"));
    };

    let io = tokio::io::join(r, w.writer);
    let mut io = Some(TokioIo::new(io));

    let (sr, conn): (SendRequest<EmptyBody>, Connection<_, _>) =
        match hyper::client::conn::http1::Builder::new()
            .handshake(io.take().unwrap())
            .now_or_never()
            .unwrap()
        {
            Ok(x) => x,
            Err(e) => {
                warn!("Failed to create http1 client: {e}");
                return Err(ctx.err("Failed to create http1 client"));
            }
        };

    tokio::spawn(async move {
        match conn.with_upgrades().await {
            Ok(()) => (),
            Err(e) => {
                error!("Error serving hyper client connection: {e}");
            }
        }
    });
    Ok(Some(Http1Client { sr, hup: c }).wrap())
}

//@ Perform WebSocket client handshake, then recover the downstream stream socket that was used for `http_client`.
fn ws_upgrade(
    ctx: NativeCallContext,
    opts: Dynamic,
    //@ Additional request headers to include
    custom_headers: rhai::Map,
    client: Handle<Http1Client>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_upgrade");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct WsUpgradeOpts {
        url: String,
        host: Option<String>,

        //@ Do not check response headers for correctness.
        //@ Note that some `Upgrade:` header is required to continue connecting.
        #[serde(default)]
        lax: bool,

        //@ Do not include any headers besides 'Host' (if any) in request
        #[serde(default)]
        omit_headers: bool,
    }
    let opts: WsUpgradeOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, url=opts.url, "options parsed");

    let Some(mut client) = client.lut() else {
        return Err(ctx.err("Null http1 client handle specified"));
    };

    let mut prng = rand_chacha::ChaCha12Rng::from_rng(&mut *the_scenario.prng.lock().unwrap());

    Ok(async move {
        let opts = opts;
        debug!("node started");

        let key = {
            let array: [u8; 16] = prng.random();
            base64::prelude::BASE64_STANDARD.encode(array)
        };

        let mut rqb = http::Request::builder().uri(opts.url);
        if !opts.omit_headers {
            rqb = rqb
                .header(header::CONNECTION, "upgrade")
                .header(header::UPGRADE, "websocket")
                .header(header::CONTENT_LENGTH, "0")
                .header(header::SEC_WEBSOCKET_VERSION, "13")
                .header(header::SEC_WEBSOCKET_KEY, key.clone());
        }
        if let Some(hh) = opts.host {
            rqb = rqb.header(header::HOST, hh);
        }
        for (chn, chv) in custom_headers {
            if chv.is_blob() {
                rqb = rqb.header(chn.as_str(), chv.into_blob().unwrap());
            } else {
                rqb = rqb.header(chn.as_str(), format!("{}", chv));
            }
        }
        let rq = rqb.body(http_body_util::Empty::<Bytes>::new())?;

        debug!("request {rq:?}");
        let resp = client.sr.send_request(rq).await?;
        debug!("response {resp:?}");

        if !opts.lax {
            if resp.status() != StatusCode::SWITCHING_PROTOCOLS {
                bail!(
                    "Upstream server returned status code other than `switching protocols`: {}",
                    resp.status()
                );
            }
            let Some(upgrval) = resp.headers().get(header::UPGRADE) else {
                bail!("Upstream server failed to return an `Upgrade` header");
            };
            if upgrval != "websocket" {
                bail!("Upstream server's Upgrade: header is not `websocket`");
            }
            let Some(upstream_accept) = resp.headers().get(header::SEC_WEBSOCKET_ACCEPT) else {
                bail!("Upstream server failed to return an `Sec-Websocket-Accept` header");
            };

            let mut keybuf = String::with_capacity(key.len() + 36);
            keybuf.push_str(&key[..]);
            keybuf.push_str(MAGIC_GUID);
            let hash = Sha1::digest(keybuf.as_bytes());
            let expected_accept = base64::prelude::BASE64_STANDARD.encode(hash);

            if upstream_accept != expected_accept.as_bytes() {
                bail!(
                    "Upstream server failed to return invalid `Sec-Websocket-Accept` header value"
                );
            }
        }

        let upg = hyper::upgrade::on(resp).await?;
        let parts = upg.downcast().unwrap();
        let io: IoType = parts.io;
        let (mut r, w) = io.into_inner().into_inner();

        let mut new_prefix = BytesMut::from(&parts.read_buf[..]);
        new_prefix.extend_from_slice(&r.prefix);
        r.prefix = new_prefix;

        let s = StreamSocket {
            read: Some(r),
            write: Some(StreamWrite { writer: w }),
            close: client.hup,
        };
        debug!(s=?s, "upgraded");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Converts a downstream stream socket into a HTTP 1 server, suitable for accepting e.g. WebSocket upgrade request.
fn http1_serve(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "http1_serve");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct Http1ServeOpts {}
    let opts: Http1ServeOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "options parsed");

    Ok(async move {
        let _opts = opts;
        debug!("node started");
        let Some(StreamSocket {
            read: Some(r),
            write: Some(w),
            close: c,
        }) = inner.lut()
        else {
            bail!("Incomplete underlying socket specified")
        };

        let server_builder = hyper::server::conn::http1::Builder::new();
        let io = tokio::io::join(r, w.writer);

        let io: IoType = TokioIo::new(io);

        let c: Handle<Hangup> = c.wrap();

        let service = hyper::service::service_fn(move |rq: IncomingRequest| {
            debug!(?rq, "request");
            let c = c.clone();
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            async move {
                let h: Handle<IncomingRequest> = Some(rq).wrap();

                let resp: Handle<OutgoingResponse> =
                    match the_scenario.callback::<Handle<OutgoingResponse>, (Handle<IncomingRequest>, Handle<Hangup>)>(
                        continuation,
                        (h, c),
                    ) {
                        Ok(x) => x,
                        Err(e) => {
                            warn!("Error from handler function: {e}");
                            return Ok(Response::builder()
                                .status(500)
                                .body(EmptyBody::new())
                                .unwrap());
                        }
                    };

                let Some(resp) = resp.lut() else {
                    warn!("Empty handle from handler function");
                    return Ok(Response::builder()
                        .status(500)
                        .body(EmptyBody::new())
                        .unwrap());
                };
                Ok::<Response<EmptyBody>, anyhow::Error>(resp)
            }
        });
        let conn = server_builder.serve_connection(io, service);
        let conn = conn.with_upgrades();

        match conn.await {
            Ok(()) => (),
            Err(e) => {
                error!("Error serving hyper client connection: {e}");
            }
        }

        Ok(())
    }
    .instrument(span)
    .wrap())
}

//@ Perform WebSocket server handshake, then recover the downstream stream socket that was used for `http_server`.
fn ws_accept(
    ctx: NativeCallContext,
    opts: Dynamic,
    custom_headers: rhai::Map,
    rq: Handle<IncomingRequest>,
    close_handle: Handle<Hangup>,
    continuation: FnPtr,
) -> RhResult<Handle<OutgoingResponse>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_accept");
    let the_scenario = ctx.get_scenario()?;
    #[derive(serde::Deserialize)]
    struct WsAcceptOpts {
        //@ Do not check incoming headers for correctness
        #[serde(default)]
        lax: bool,

        //@ Do not include any headers in response
        #[serde(default)]
        omit_headers: bool,

        //@ If client supplies Sec-WebSocket-Protocol and it contains this one,
        //@ include the header in response.
        choose_protocol: Option<String>,

        //@ Fail incoming connection if Sec-WebSocket-Protocol lacks the value specified in
        //@ `choose_protocol` field (or any protocol if `protocol_choose_first` is active).
        #[serde(default)]
        require_protocol: bool,

        //@ Round trip Sec-WebSocket-Protocol from request to response,
        //@ choosing the first protocol if there are multiple
        #[serde(default)]
        protocol_choose_first: bool,
    }
    let opts: WsAcceptOpts = rhai::serde::from_dynamic(&opts)?;

    if opts.require_protocol && opts.choose_protocol.is_none() {
        return Err(ctx.err("`require_protocol` cannot be used without `choose_protocol`"));
    }

    debug!(parent: &span, "options parsed");

    let c: Option<Hangup> = close_handle.lut();
    let Some(rq) = rq.lut() else {
        return Err(ctx.err("Null request token specified"));
    };

    let bail = || -> RhResult<Handle<OutgoingResponse>> {
        let response = Response::builder()
            .status(400)
            .body(EmptyBody::new())
            .unwrap();
        Ok(Some(response).wrap())
    };
    if !opts.lax {
        if rq.method() != http::Method::GET {
            warn!("Incoming WebSocket connection's method is not GET");
            return bail();
        }
        let Some(upgrval) = rq.headers().get(header::UPGRADE) else {
            warn!("Incoming WebSocket connection's lacks Upgrade: header");
            return bail();
        };
        if upgrval != "websocket" {
            warn!("Incoming WebSocket connection's Upgrade: header is not `websocket`");
            return bail();
        }
        let Some(wsver) = rq.headers().get(header::SEC_WEBSOCKET_VERSION) else {
            warn!("Incoming WebSocket connection's lacks Sec-Websocket-Version: header");
            return bail();
        };
        if wsver != "13" {
            warn!("Incoming WebSocket connection's  Sec-Websocket-Version: header is not `13`");
            return bail();
        }
    }
    let chosen_protocol = 'proto_chose: {
        if opts.choose_protocol.is_some() || opts.protocol_choose_first {
            if let Some(p) = rq.headers().get(header::SEC_WEBSOCKET_PROTOCOL) {
                for pp in p.as_bytes().split(|&x| x == b',') {
                    let pp = pp.trim_ascii();
                    if Some(pp) == opts.choose_protocol.as_ref().map(|x| x.as_bytes()) {
                        break 'proto_chose Some(pp.to_owned());
                    }
                }
                if opts.protocol_choose_first {
                    let mut protocols = p.as_bytes().split(|&x| x == b',');
                    if let Some(first_protocol) = protocols.next() {
                        let first_protocol = first_protocol.trim_ascii();
                        break 'proto_chose Some(first_protocol.to_owned());
                    }
                }
                None
            } else {
                None
            }
        } else {
            None
        }
    };
    if chosen_protocol.is_none() && opts.require_protocol {
        warn!("Incoming WebSocket connection lacks Sec-Websocket-Protocol value");
        return bail();
    }

    let mut response_builder = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
    if !opts.omit_headers {
        response_builder = response_builder
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket");

        match rq.headers().get(header::SEC_WEBSOCKET_KEY) {
            Some(wskey) => {
                let mut concat_key = Vec::with_capacity(wskey.len() + 36);
                concat_key.extend_from_slice(wskey.as_bytes());
                concat_key.extend_from_slice(MAGIC_GUID.as_bytes());
                let hash = Sha1::digest(concat_key);

                let accept = base64::engine::general_purpose::STANDARD.encode(hash);

                response_builder = response_builder.header(header::SEC_WEBSOCKET_ACCEPT, accept);
            }
            None if !opts.lax => {
                warn!("Incoming WebSocket connection's lacks Sec-Websocket-Key: header");
                return bail();
            }
            None => {
                debug!("No Sec-Websocket-Key header, so replying  without Sec-Websocket-Accept.")
            }
        }
    }

    for (chn, chv) in custom_headers {
        if chv.is_blob() {
            response_builder = response_builder.header(chn.as_str(), chv.into_blob().unwrap());
        } else {
            response_builder = response_builder.header(chn.as_str(), format!("{}", chv));
        }
    }
    if let Some(chp) = chosen_protocol {
        response_builder = response_builder.header(header::SEC_WEBSOCKET_PROTOCOL, chp);
    }

    let response = response_builder.body(EmptyBody::new()).unwrap();
    debug!(resp=?response, "response");

    tokio::spawn(async move {
        let upg = match hyper::upgrade::on(rq).await {
            Ok(x) => x,
            Err(e) => {
                error!("Error accepting WebSocket conenction: {e}");
                return;
            }
        };

        let parts = match upg.downcast() {
            Ok(x) => x,
            Err(_e) => {
                error!("Error downcasting Upgraded");
                return;
            }
        };
        let io: IoType = parts.io;

        let (mut r, w) = io.into_inner().into_inner();

        let mut new_prefix = BytesMut::from(&parts.read_buf[..]);
        new_prefix.extend_from_slice(&r.prefix);
        r.prefix = new_prefix;

        let s = StreamSocket {
            read: Some(r),
            write: Some(StreamWrite { writer: w }),
            close: c,
        };
        debug!(s=?s, "accepted");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
    });

    Ok(Some(response).wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("http1_client", http1_client);
    engine.register_fn("ws_upgrade", ws_upgrade);
    engine.register_fn("http1_serve", http1_serve);
    engine.register_fn("ws_accept", ws_accept);
}
