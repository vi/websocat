use anyhow::bail;
use bytes::Bytes;
use http::{header, StatusCode};
use hyper::client::conn::http1::{Connection, SendRequest};
use hyper_util::rt::TokioIo;
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, error, field, Instrument};

use crate::scenario_executor::{scenario::{callback_and_continue, ScenarioAccess}, types::{Handle, StreamRead, StreamSocket, StreamWrite, Task}, utils::{Anyhow2EvalAltResult, HandleExt2, TaskHandleExt2}};

fn ws_upgrade(
    ctx: NativeCallContext,
    inner: Handle<StreamSocket>,
    opts: Dynamic,
    continuation: FnPtr,
) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "ws_upgrade", addr = field::Empty);
    let the_scenario = ctx.get_scenario().tbar()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct WsUpgradeOpts {
        url: String,
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

        // FIXME: read debt
        assert!(r.prefix.is_empty());
        let io = tokio::io::join(r.reader, w.writer);
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

        let rq = http::Request::builder()
            .uri(opts.url)
            .header(header::HOST, "localhost") // FIXME: de-hardcode
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, "r2uF3+29PMsvBbhFKbt66A==") // FIXME de-hardcode
            .header(header::CONTENT_LENGTH, "0")
            .body(http_body_util::Empty::<Bytes>::new())?;


        let resp = sr.send_request(rq).await?;


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
        let Some(_upstream_accept) = resp.headers().get(header::SEC_WEBSOCKET_ACCEPT) else {
            bail!("Upstream server failed to return an `Sec-Websocket-Accept` header");
        };

        // FIXME: actually check accept value

        /*if upstream_accept != x.expected_accept.as_bytes() {
            bail!("Upstream server failed to return invalid `Sec-Websocket-Accept` header value");
        }*/

        let upg = hyper::upgrade::on(resp).await?;
        let parts = upg.downcast().unwrap();
        io = Some(parts.io);
        let (r,w) = io.unwrap().into_inner().into_inner();

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: parts.read_buf,
            }),
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
