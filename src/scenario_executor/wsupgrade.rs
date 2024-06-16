use anyhow::bail;
use base64::Engine as _;
use bytes::{Bytes, BytesMut};
use http::{header, StatusCode};
use hyper::client::conn::http1::{Connection, SendRequest};
use hyper_util::rt::TokioIo;
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use sha1::{Digest, Sha1};
use tracing::{debug, debug_span, error, field, Instrument};

use crate::scenario_executor::{scenario::{callback_and_continue, ScenarioAccess}, types::{Handle, StreamSocket, StreamWrite, Task}, utils::{HandleExt2, TaskHandleExt2}};

static MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn ws_upgrade(
    ctx: NativeCallContext,
    inner: Handle<StreamSocket>,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_upgrade", addr = field::Empty);
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
    span.record("url", field::display(opts.url.clone()));
    debug!(parent: &span, "options parsed");

    Ok(async move {
        let opts = opts;
        debug!("node started");
        let Some(StreamSocket {
                read: Some(r),
                write: Some(w),
                close: c,
            }) = inner.lut() else {
                bail!("Incomplete underlying socket specified")
            };

        let io = tokio::io::join(r, w.writer);
        let mut io = Some(TokioIo::new(io));

        let (mut sr, conn): (SendRequest<http_body_util::Empty<Bytes>>, Connection<_, _>) =
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
            let array : [u8; 16] = rand::random();
            base64::prelude::BASE64_STANDARD.encode(array)
        };

        let mut rqb =  http::Request::builder()
            .uri(opts.url)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::CONTENT_LENGTH, "0")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, key.clone());
        if let Some(hh) = opts.host {
            rqb = rqb.header(header::HOST, hh);
        }
        let rq = rqb
            .body(http_body_util::Empty::<Bytes>::new())?;


        debug!("request {rq:?}");
        let resp = sr.send_request(rq).await?;
        debug!("response {resp:?}");

        if ! opts.lax {
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
                bail!("Upstream server failed to return invalid `Sec-Websocket-Accept` header value");
            }
        }

        let upg = hyper::upgrade::on(resp).await?;
        let parts = upg.downcast().unwrap();
        io = Some(parts.io);
        let (mut r,w) = io.unwrap().into_inner().into_inner();

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

        callback_and_continue(the_scenario, continuation, (h,))
            .await;
        Ok(())
    }.instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_upgrade", ws_upgrade);
}
