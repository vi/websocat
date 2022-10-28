#![allow(unused)]
use futures::StreamExt;
use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, http, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;
#[derive(Debug, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-server", validate)]
#[auto_populate_in_allclasslist]
#[derivative(Clone)]
pub struct HttpServer {
    /// IO bytestream node to use
    inner: NodeId,

    /// Expect and handle upgrades
    #[websocat_prop(default = false)]
    upgrade: bool,

    /// Expect and handle specifically WebSocket upgrades
    #[websocat_prop(default = false)]
    websocket: bool,
}

impl HttpServer {
    fn validate(&mut self) -> Result<()> {
        if self.websocket {
            self.upgrade = true;
        }
        Ok(())
    }
}

async fn handle_request(
    rq: hyper::Request<hyper::Body>,
    tx: Option<tokio::sync::oneshot::Sender<(websocat_api::Source, websocat_api::Sink)>>,
) -> Result<hyper::Response<hyper::Body>> {
    tracing::info!("rq: {:?}", rq);
    if let (Some(tx)) = tx {
        let mut incoming_body = rq.into_body();
        let (sender, outgoing_body) = hyper::Body::channel();
        let sink = crate::util::body_sink(sender);

        let (request_tx, request_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
        while let Some(buf) = incoming_body.next().await {
            request_tx.send(buf?).await?;
        }

        let source = crate::util::body_source(request_rx);

        tx.send((
            websocat_api::Source::Datagrams(Box::pin(source)),
            websocat_api::Sink::Datagrams(Box::pin(sink)),
        ));

        incoming_body;
        Ok(hyper::Response::new(outgoing_body))
    } else {
        anyhow::bail!("Trying to reuse HTTP connection for second request to Websocat, which is not supported in this mode")
    }
}
async fn handle_request_for_upgrade(
    rq: hyper::Request<hyper::Body>,
    tx: Option<tokio::sync::oneshot::Sender<(websocat_api::Source, websocat_api::Sink)>>,
    websocket_mode: bool,
) -> Result<hyper::Response<hyper::Body>> {
    tracing::info!("rq: {:?}", rq);

    let wskey = if websocket_mode {
        let hh = rq.headers();
        if let (Some(c), Some(u), Some(v), Some(k)) = (
            hh.get(hyper::header::CONNECTION),
            hh.get(hyper::header::UPGRADE),
            hh.get(hyper::header::SEC_WEBSOCKET_VERSION),
            hh.get(hyper::header::SEC_WEBSOCKET_KEY),
        ) {
            if !c.as_bytes().eq_ignore_ascii_case(b"upgrade") {
                anyhow::bail!("http-server is in websocket mode `Connection:` is not `upgrade`");
            }
            if !u.as_bytes().eq_ignore_ascii_case(b"websocket") {
                anyhow::bail!("http-server is in websocket mode `Upgrade:` is not `websocket`");
            }
            if !v.as_bytes().eq_ignore_ascii_case(b"13") {
                anyhow::bail!(
                    "http-server is in websocket mode `Sec-WebSocket-Version` is not `13`"
                );
            }
            let accept = crate::util::derive_websocket_accept_key(k.as_bytes());
            Some(accept)
        } else {
            anyhow::bail!("http-server is in websocket mode and some of the four websocekt response headers are not found")
        }
    } else {
        None
    };

    if let (Some(tx)) = tx {
        let upg = hyper::upgrade::on(rq);
        tokio::spawn(async {
            match upg.await {
                Ok(upg) => {
                    let (r, w) = tokio::io::split(upg);
                    // TODO: also try downcast somehow, like in the client
                    tx.send((
                        websocat_api::Source::ByteStream(Box::pin(r)),
                        websocat_api::Sink::ByteStream(Box::pin(w)),
                    ));
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    drop(tx);
                }
            }
        });
        let mut resp = hyper::Response::new(hyper::Body::empty());
        resp.headers_mut().append(
            http::header::CONNECTION,
            http::HeaderValue::from_static("upgrade"),
        );

        if let Some(wskey) = wskey {
            resp.headers_mut().append(
                http::header::UPGRADE,
                http::HeaderValue::from_static("websocket"),
            );
            resp.headers_mut().append(
                http::header::SEC_WEBSOCKET_VERSION,
                http::HeaderValue::from_static("13"),
            );
            resp.headers_mut().append(
                http::header::SEC_WEBSOCKET_ACCEPT,
                http::HeaderValue::from_str(&wskey).unwrap(),
            );
        }

        *resp.status_mut() = hyper::StatusCode::SWITCHING_PROTOCOLS;
        tracing::debug!("resp: {:?}", resp);
        Ok(resp)
    } else {
        anyhow::bail!("Trying to reuse HTTP connection for second request to Websocat, which is not supported in this mode")
    }
}

#[async_trait]
impl websocat_api::RunnableNode for HttpServer {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: websocat_api::RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<websocat_api::Bipipe> {
        let mut io = None;
        let mut cn = None;

        let io_ = ctx.nodes[self.inner]
            .clone()
            .upgrade()?
            .run(ctx.clone(), multiconn)
            .await?;
        cn = io_.closing_notification;
        io = Some(match (io_.r, io_.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("HTTP server requires a bytestream-based inner node");
            }
        });

        let (tx, rx) = tokio::sync::oneshot::channel();

        let mut tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

        let http = hyper::server::conn::Http::new();

        if !self.upgrade {
            let service = hyper::service::service_fn(move |rq| {
                let tx = tx.clone().lock().unwrap().take();
                handle_request(rq, tx)
            });
            let conn = http.serve_connection(io.unwrap(), service);

            use websocat_api::futures::TryFutureExt;
            tokio::spawn(conn.map_err(|e| {
                tracing::error!("hyper server error: {}", e);
                ()
            }));
        } else {
            let service = hyper::service::service_fn(move |rq| {
                let tx = tx.clone().lock().unwrap().take();
                handle_request_for_upgrade(rq, tx, self.websocket)
            });
            let conn = http.serve_connection(io.unwrap(), service);

            let conn = conn.with_upgrades();

            use websocat_api::futures::TryFutureExt;
            tokio::spawn(conn.map_err(|e| {
                tracing::error!("hyper server error: {}", e);
                ()
            }));
        }

        let (r, w) = rx.await?;

        Ok(websocat_api::Bipipe {
            r,
            w,
            closing_notification: cn,
        })
    }
}

#[derive(Debug, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-server2")]
#[auto_populate_in_allclasslist]
#[derivative(Clone)]
pub struct HttpServer2 {
    /// IO bytestream node to use
    inner: NodeId,
}

#[async_trait]
impl websocat_api::RunnableNode for HttpServer2 {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: websocat_api::RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<websocat_api::Bipipe> {
        let mut io = None;
        let mut cn = None;

        let io_ = ctx.nodes[self.inner]
            .clone()
            .upgrade()?
            .run(ctx.clone(), multiconn)
            .await?;
        cn = io_.closing_notification;
        io = Some(match (io_.r, io_.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("HTTP server requires a bytestream-based inner node");
            }
        });

        let (rq_tx, rq_rx) =
            tokio::sync::mpsc::channel::<Result<websocat_api::HttpRequestWithAResponseSlot>>(1);

        let http = hyper::server::conn::Http::new();

        let service = hyper::service::service_fn(move |rq| {
            let rq_tx = rq_tx.clone();
            async move {
                let (rs_tx, rs_rx) = tokio::sync::oneshot::channel();
                if let Err(e) = rq_tx.send(Ok((rq, rs_tx))).await {
                    tracing::error!("Failed to send request to the outer node");
                    return Ok::<_, anyhow::Error>(
                        hyper::Response::builder()
                            .status(500)
                            .body(hyper::Body::empty())?,
                    );
                }
                match rs_rx.await {
                    Ok(x) => Ok(x),
                    Err(_) => {
                        tracing::error!(
                            "Failed to receive any kind of response from the outer node"
                        );
                        Ok(hyper::Response::builder()
                            .status(500)
                            .body(hyper::Body::empty())?)
                    }
                }
            }
        });
        let conn = http.serve_connection(io.unwrap(), service);

        let conn = conn.with_upgrades();

        use websocat_api::futures::TryFutureExt;
        tokio::spawn(conn.map_err(|e| {
            tracing::error!("hyper server error: {}", e);
            ()
        }));

        let r = futures::stream::unfold(rq_rx, move |mut rq_rx| async move {
            let x: Option<Result<websocat_api::HttpRequestWithAResponseSlot>> = rq_rx.recv().await;
            if x.is_none() {
                tracing::debug!("HTTP request stream is finished");
            }
            x.map(move |rq| {
                if let Ok(ref rq) = rq {
                    tracing::debug!("Incoming HTTP request {} {}", rq.0.method(), rq.0.uri());
                } else {
                    tracing::error!(
                        "Error instead of HTTP request? This should be an unreacahble message"
                    );
                }
                (rq, rq_rx)
            })
        });

        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::Http(Box::pin(r)),
            w: websocat_api::Sink::None,
            closing_notification: cn,
        })
    }
}
