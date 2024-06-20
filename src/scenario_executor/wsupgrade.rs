use anyhow::bail;
use base64::Engine as _;
use bytes::{Bytes, BytesMut};
use http::{header, Response, StatusCode};
use hyper::client::conn::http1::{Connection, SendRequest};
use hyper_util::rt::TokioIo;
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWrite;
use tracing::{debug, debug_span, error, field, warn, Instrument};
use std::{pin::Pin, sync::Arc};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, Hangup, StreamRead, StreamSocket, StreamWrite, Task},
    utils::{HandleExt, HandleExt2, TaskHandleExt2},
};

type EmptyBody = http_body_util::Empty<bytes::Bytes>;

static MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn ws_upgrade(
    ctx: NativeCallContext,
    inner: Handle<StreamSocket>,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_upgrade");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct WsUpgradeOpts {
        url: String,
        host: Option<String>,
        #[serde(default)]
        lax: bool,
    }
    let opts: WsUpgradeOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, url=opts.url, "options parsed");

    Ok(async move {
        let opts = opts;
        debug!("node started");
        let Some(StreamSocket {
            read: Some(r),
            write: Some(w),
            close: c,
        }) = inner.lut()
        else {
            bail!("Incomplete underlying socket specified")
        };

        let io = tokio::io::join(r, w.writer);
        let mut io = Some(TokioIo::new(io));

        let (mut sr, conn): (SendRequest<EmptyBody>, Connection<_, _>) =
            hyper::client::conn::http1::Builder::new()
                .handshake(io.take().unwrap())
                .await?;

        tokio::spawn(async move {
            match conn.with_upgrades().await {
                Ok(()) => (),
                Err(e) => {
                    error!("Error serving hyper client connection: {e}");
                }
            }
        });

        let key = {
            let array: [u8; 16] = rand::random();
            base64::prelude::BASE64_STANDARD.encode(array)
        };

        let mut rqb = http::Request::builder()
            .uri(opts.url)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::CONTENT_LENGTH, "0")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, key.clone());
        if let Some(hh) = opts.host {
            rqb = rqb.header(header::HOST, hh);
        }
        let rq = rqb.body(http_body_util::Empty::<Bytes>::new())?;

        debug!("request {rq:?}");
        let resp = sr.send_request(rq).await?;
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
        io = Some(parts.io);
        let (mut r, w) = io.unwrap().into_inner().into_inner();

        let mut new_prefix = BytesMut::from(&parts.read_buf[..]);
        new_prefix.extend_from_slice(&r.prefix);
        r.prefix = new_prefix;

        let s = StreamSocket {
            read: Some(r),
            write: Some(StreamWrite { writer: w }),
            close: c,
        };
        debug!(s=?s, "upgraded");
        let h = s.wrap();

        callback_and_continue(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

fn ws_accept(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_accept");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct WsAcceptOpts {
        #[serde(default)]
        lax: bool,
    }
    let opts: WsAcceptOpts = rhai::serde::from_dynamic(&opts)?;
    debug!(parent: &span, "options parsed");

    Ok(async move {
        let opts = opts;
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

        type IoType = TokioIo<tokio::io::Join<StreamRead, Pin<Box<dyn AsyncWrite + Send>>>>;
        let io: IoType  = TokioIo::new(io);

        let c : Handle<Hangup> = c.wrap();

        let service = hyper::service::service_fn(move |rq: hyper::Request<hyper::body::Incoming>| {
            debug!(?rq, "request");
            let c = c.clone();
            let the_scenario = the_scenario.clone();
            let continuation = continuation.clone();
            async move {
                let bail = || -> anyhow::Result<Response<EmptyBody>> {
                    let response = Response::builder()
                        .status(400)
                        .body(EmptyBody::new())
                        .unwrap();
                    Ok(response)
                };
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
                    warn!("Incoming WebSocket connection's lacks Websocket-Sec-Version: header");
                    return bail();
                };
                if wsver != "13" {
                    warn!("Incoming WebSocket connection's  Websocket-Sec-Version: header is not `13`");
                    return bail();
                }
                let Some(wskey) = rq.headers().get(header::SEC_WEBSOCKET_KEY) else {
                    warn!("Incoming WebSocket connection's lacks Websocket-Sec-Key: header");
                    return bail();
                };
                /*let Ok(key) = base64::engine::general_purpose::STANDARD.decode(wskey) else {
                    warn!("Incoming WebSocket connection's Websocket-Sec-Key: is invalid base64");
                    return bail();
                };
                if key.len() != 20 {
                    warn!("Incoming WebSocket connection's Websocket-Sec-Key: is not exactly 20 base64-encoded bytes");
                    return bail();
                }
                let mut array = [0u8; 20];
                array[..20].clone_from_slice(&key[..20]);
                */

                let mut concat_key = Vec::with_capacity(wskey.len() + 36);
                concat_key.extend_from_slice(wskey.as_bytes());
                concat_key.extend_from_slice(MAGIC_GUID.as_bytes());
                let hash = Sha1::digest(concat_key);

                let accept = base64::engine::general_purpose::STANDARD.encode(hash);

                let response = Response::builder()
                    .status(StatusCode::SWITCHING_PROTOCOLS)
                    .header(header::CONNECTION, "upgrade")
                    .header(header::UPGRADE, "websocket")
                    .header(header::SEC_WEBSOCKET_ACCEPT, accept)
                    .body(EmptyBody::new())
                    .unwrap();
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
                    let io : IoType = parts.io;

                    let (mut r, w) = io.into_inner().into_inner();

                    let mut new_prefix = BytesMut::from(&parts.read_buf[..]);
                    new_prefix.extend_from_slice(&r.prefix);
                    r.prefix = new_prefix;

                    let s = StreamSocket {
                        read: Some(r),
                        write: Some(StreamWrite { writer: w }),
                        close: c.lut(),
                    };
                    debug!(s=?s, "accepted");
                    let h = s.wrap();

                    callback_and_continue(the_scenario, continuation, (h,)).await;

                });



                Ok::<Response<EmptyBody>, anyhow::Error>(response)
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


pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_upgrade", ws_upgrade);
    engine.register_fn("ws_accept", ws_accept);
}
