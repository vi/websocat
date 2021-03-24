use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;
#[derive(Debug, Clone, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-client", validate)]
pub struct HttpClient {
    /// IO object to use for HTTP1 handshake
    inner: NodeId,

    /// Expect and work upon upgrades
    #[websocat_prop(default=false)]
    upgrade: bool,

    /// Immediately return connection, stream bytes into request body.
    #[websocat_prop(default=false)]
    stream_request_body: bool,

    /// Subnode to read request body from
    request_body: Option<NodeId>,

    /// Fully read request body into memory prior to sending it
    #[websocat_prop(default=false)]
    buffer_request_body: bool,

    /// Preallocate this amount of memory for caching request body
    #[websocat_prop(default=1024, min=1, reasonable_max=100_000_000)]
    buffer_request_body_size_hint: i64,

    /// Override HTTP request verb
    method: Option<String>,

    /// Set request content_type to `application/json`
    //#[cli="json"]
    #[websocat_prop(default=false)]
    json: bool, 

    /// Set request content_type to `text/plain`
    //#[cli="json"]
    #[websocat_prop(default=false)]
    textplain: bool, 

    /// Add these headers to HTTP request
    request_headers: Vec<NodeId>,

    /// Request URI
    uri : Option<websocat_api::http::Uri>,

    /// Request WebSocket upgrade from server
    #[websocat_prop(default=false)]
    websocket: bool,
}

impl HttpClient {
    fn validate(&mut self) -> Result<()> {
        if self.stream_request_body {
            if self.upgrade {
                anyhow::bail!(
                    "Cannot set both `upgrade` and `stream_request_body` options at the same time"
                );
            }
            if self.request_body.is_some() {
                anyhow::bail!(
                    "Cannot set both `body` and `stream_request_body` options at the same time"
                );
            }
        }

        if self.buffer_request_body
            && (self.request_body.is_none() && !self.stream_request_body)
        {
            anyhow::bail!("buffer_request_body option is meaningless withouth stream_request_body or request_body options");
        }

        if !self.buffer_request_body  && self.buffer_request_body_size_hint != 1024  {
            anyhow::bail!("buffer_request_body_size_hint option is meaningless withouth buffer_request_body option");
        }

        if let Some(ref verb) = self.method {
            let _ = hyper::Method::from_bytes(verb.as_bytes())?;
        }

        if self.textplain && self.json {
            anyhow::bail!("Cannot set both textplain and options to true");
        }

        if self.stream_request_body && self.websocket {
            anyhow::bail!("stream_request_body and websocket options are incompatible");
        }

        if self.websocket {
            self.upgrade = true;
        }

        Ok(())
    }
}

/// Derive the `Sec-WebSocket-Accept` response header from a `Sec-WebSocket-Key` request header.
///
/// This function can be used to perform a handshake before passing a raw TCP stream to
/// [`WebSocket::from_raw_socket`][crate::protocol::WebSocket::from_raw_socket].
///
/// Based on https://github.com/snapview/tungstenite-rs/blob/985d6571923c2eac3310d8a9981a2306ae675214/src/handshake/mod.rs#L113
fn derive_accept_key(request_key: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::default();
    sha1.update(request_key);
    sha1.update(WS_GUID);
    base64::encode(&sha1.finalize())
}

impl HttpClient {
    fn get_request(&self, body: hyper::Body, ctx: &websocat_api::RunContext) -> Result<(hyper::Request<hyper::Body>, Option<[u8;16]>)> {
        let mut rq = hyper::Request::new(body);
        let mut thekey = None;

        if self.websocket {
            let r: [u8; 16] = rand::random();
            let key = base64::encode(&r);
            thekey = Some(r);

            rq.headers_mut().insert(hyper::header::CONNECTION, "Upgrade".parse().unwrap());
            rq.headers_mut().insert(hyper::header::UPGRADE, "websocket".parse().unwrap());
            rq.headers_mut().insert(hyper::header::SEC_WEBSOCKET_VERSION, "13".parse().unwrap());
            rq.headers_mut().insert(hyper::header::SEC_WEBSOCKET_KEY, key.parse().unwrap());
        }

        if let Some(ref verb) = self.method {
            *rq.method_mut() = hyper::Method::from_bytes(verb.as_bytes())?;
        } else if self.request_supposed_to_contain_body() {
            *rq.method_mut() = hyper::Method::POST;
        }
        if self.json {
            rq.headers_mut().insert(hyper::header::CONTENT_TYPE, "application/json".parse().unwrap());
        }
        if self.textplain {
            rq.headers_mut().insert(hyper::header::CONTENT_TYPE, "text/plain".parse().unwrap());
        }

        for h in &self.request_headers {
            use websocat_api::PropertyValue;
            let h = &*ctx.nodes[h];
            if let (Some(PropertyValue::Stringy(n)), Some(PropertyValue::Stringy(v))) = (h.get_property("n"), h.get_property("v")) {
                rq.headers_mut().insert(
                    hyper::header::HeaderName::from_bytes(n.as_bytes())?, 
                    hyper::header::HeaderValue::from_bytes(v.as_bytes())?,
                );
            } else {
                anyhow::bail!("http-client's array elements must be `header` nodes");
            }
        }

        if let Some(ref uri) = self.uri {
            *rq.uri_mut() = uri.clone();
        }

        Ok((rq, thekey))
    }

    fn handle_response(&self, resp: &hyper::Response<hyper::Body>, wskey: Option<[u8; 16]>) -> Result<()> {
        tracing::debug!("Response status: {}", resp.status());
        for h in resp.headers() {
            tracing::debug!("Response header: {}={:?}", h.0, h.1);
        }
        if let Some(key) = wskey {
            let hh = resp.headers();
            if let (Some(c), Some(u), Some(a)) = (hh.get(hyper::header::CONNECTION), hh.get(hyper::header::UPGRADE), hh.get(hyper::header::SEC_WEBSOCKET_ACCEPT)) {
                if ! c.as_bytes().eq_ignore_ascii_case(b"upgrade") {
                    anyhow::bail!("http-client is in websocket mode `Connection:` is not `upgrade`");
                }
                if ! u.as_bytes().eq_ignore_ascii_case(b"websocket") {
                    anyhow::bail!("http-client is in websocket mode `Upgrade:` is not `websocket`");
                }
                let accept = derive_accept_key(base64::encode(key).as_bytes());
                if accept != String::from_utf8_lossy(a.as_bytes()) {
                    anyhow::bail!("Sec-Websocket-Accept key mismatch: expected {}, got {:?}", accept, a);
                }
            } else {
                anyhow::bail!("http-client is in websocket mode and some of the three websocekt response headers are not found");
            }
            tracing::debug!("WebSocket client response verification finished");
        }
        Ok(())
    }

    fn request_supposed_to_contain_body(&self) -> bool {
        self.stream_request_body || self.request_body.is_some()
    }
}

#[async_trait]
impl websocat_api::Node for HttpClient {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: websocat_api::RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {
        let io = ctx.nodes[self.inner].clone().run(ctx.clone(), multiconn).await?;
        let cn = io.closing_notification;
        let mut io = Some(match (io.r, io.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("HTTP client requires bytestream-based inner node");
            }
        });

        let http_client_builder = hyper::client::conn::Builder::new().handshake::<_, hyper::Body>(io.take().unwrap());
        let (mut send_request, conn) = http_client_builder.await?;
        let _h = tokio::spawn(conn /* .without_shutdown() */);

        if self.stream_request_body {
            let (response_tx, response_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
            let w: websocat_api::Sink = if !self.buffer_request_body {
                // Chunked request body
                let (sender, request_body) = hyper::Body::channel();
                let sink = futures::sink::unfold(
                    sender,
                    move |mut sender, buf: bytes::Bytes| async move {
                        tracing::trace!("Sending {} bytes chunk as HTTP request body", buf.len());
                        sender.send_data(buf).await.map_err(|e| {
                            tracing::error!("Failed sending more HTTP request body: {}", e);
                            e
                        })?;
                        Ok(sender)
                    },
                );

                tokio::spawn(async move {
                    let try_block = async move {
                        let rq = self.get_request(request_body, &ctx)?.0;
                        let resp = send_request.send_request(rq).await?;
                        self.handle_response(&resp, None)?;
                        let mut body = resp.into_body();
                        use futures::stream::StreamExt;
                        while let Some(buf) = body.next().await {
                            response_tx.send(buf?).await?;
                        }
                        tracing::debug!("Finished sending streamed response");
                        Ok::<_, anyhow::Error>(())
                    };
                    if let Err(e) = try_block.await {
                        tracing::error!("streamed-http-client error: {}", e);
                    }
                });
                websocat_api::Sink::Datagrams(Box::pin(sink))
            } else {
                // Fully buffered request body
                let bufbuf = bytes::BytesMut::with_capacity(
                    self.buffer_request_body_size_hint as usize,
                );
                let (tx, rx) = tokio::sync::oneshot::channel();
                struct SendawayDropper<T>(Option<T>, Option<tokio::sync::oneshot::Sender<T>>);
                impl<T> Drop for SendawayDropper<T> {
                    fn drop(&mut self) {
                        let x: T = self.0.take().unwrap();
                        if let Err(_) = self.1.take().unwrap().send(x) {
                            tracing::error!("Failed to deliver hyper::Body to the appropiate task")
                        } else {
                            tracing::debug!("Finished buffering the hyper::Body")
                        }
                    }
                }

                let bufbufw = SendawayDropper(Some(bufbuf), Some(tx));

                let sink = futures::sink::unfold(
                    bufbufw,
                    move |mut bufbufw, buf: bytes::Bytes| async move {
                        tracing::trace!(
                            "Adding {} bytes chunk to cached HTTP request body",
                            buf.len()
                        );
                        bufbufw.0.as_mut().unwrap().extend(buf);
                        Ok(bufbufw)
                    },
                );
                tokio::spawn(async move {
                    let try_block = async move {
                        let request_buf = rx.await?;
                        let rq = self.get_request(request_buf.freeze().into(), &ctx)?.0;
                        let resp = send_request.send_request(rq).await?;
                        self.handle_response(&resp, None)?;
                        let mut body = resp.into_body();
                        use futures::stream::StreamExt;
                        while let Some(buf) = body.next().await {
                            response_tx.send(buf?).await?;
                        }
                        tracing::debug!("Finished sending streamed response");
                        Ok::<_, anyhow::Error>(())
                    };
                    if let Err(e) = try_block.await {
                        tracing::error!("streamed-http-client error: {}", e);
                    }
                });
                websocat_api::Sink::Datagrams(Box::pin(sink))
            };

            let rx = futures::stream::unfold(response_rx, move |mut response_rx| async move {
                let maybe_buf: Option<bytes::Bytes> = response_rx.recv().await;
                if maybe_buf.is_none() {
                    tracing::debug!("HTTP response body finished");
                }
                maybe_buf.map(move |buf| {
                    tracing::trace!("Sending {} bytes chunk as HTTP response body", buf.len());
                    ((Ok(buf), response_rx))
                })
            });
            Ok(websocat_api::Bipipe {
                r: websocat_api::Source::Datagrams(Box::pin(rx)),
                w,
                closing_notification: cn,
            })
        } else {
            // body is not received from upstream in this mode
            let rqbody = if let Some(ref bnid) = self.request_body {
                let bio = ctx.nodes[bnid].clone().run(ctx.clone(), None).await?;
                drop(bio.w);
                drop(bio.closing_notification);
                if self.buffer_request_body {
                    match bio.r {
                        websocat_api::Source::ByteStream(mut bs) => {
                            use tokio::io::AsyncReadExt;
                            let mut bufbuf = Vec::with_capacity(
                                self.buffer_request_body_size_hint as usize,
                            );
                            bs.read_to_end(&mut bufbuf).await?;
                            bufbuf.into()
                        }
                        websocat_api::Source::Datagrams(x) => {
                            let mut bufbuf = bytes::BytesMut::with_capacity(
                                self.buffer_request_body_size_hint as usize,
                            );
                            use futures::StreamExt;
                            let sink = futures::sink::unfold(
                                &mut bufbuf,
                                move |bufbuf: &mut bytes::BytesMut, buf: bytes::Bytes| async move {
                                    tracing::trace!(
                                        "Adding {} bytes chunk to cached HTTP request body",
                                        buf.len()
                                    );
                                    bufbuf.extend(buf);
                                    Ok::<_,anyhow::Error>(bufbuf)
                                },
                            );
                            x.forward(sink).await?;
                            tracing::debug!("Finished buffering up HTTP request body");
                            bufbuf.freeze().into()
                        }
                        websocat_api::Source::None => {
                            tracing::warn!("Unusable http-client's request_body subnode specifier: null source");
                            hyper::Body::empty() 
                        }
                    }
                } else {
                    // non-buffered body
                    let (sender, body) = hyper::Body::channel();

                    let sink = futures::sink::unfold(
                        sender,
                        move |mut sender, buf: bytes::Bytes| async move {
                            tracing::trace!("Sending {} bytes chunk as HTTP request body", buf.len());
                            sender.send_data(buf).await.map_err(|e| {
                                tracing::error!("Failed sending more HTTP request body: {}", e);
                                e
                            })?;
                            Ok(sender)
                        },
                    );

                    match bio.r {
                        websocat_api::Source::ByteStream(_) => {
                            anyhow::bail!("Use datagram-based subnode for HTTP request body. You may want to wrap it in `[datagrams inner=[...]]` or use buffer_request_body setting.")
                        }
                        websocat_api::Source::Datagrams(x) => {
                            use futures::StreamExt;
                            tokio::spawn(async move {
                                if let Err(e) = x.forward(sink).await {
                                    tracing::error!("Error forwarding chunked http request body from subnode to hyper: {}", e);
                                }
                            });
                            body
                        }
                        websocat_api::Source::None => {
                            tracing::warn!("Unusable http-client's request_body subnode specifier: null source");
                            hyper::Body::empty() 
                        }
                    }
                }
            } else {
                 hyper::Body::empty() 
            };
            let (rq, wskey) = self.get_request(rqbody, &ctx)?;

            let resp = send_request.send_request(rq).await?;
            self.handle_response(&resp, wskey)?;

            if self.upgrade {
                let upg = hyper::upgrade::on(resp).await?;
                let tmp = upg.downcast().unwrap();
                let readbuf = tmp.read_buf;

                io = Some(tmp.io);

                let (mut r, w) = io.unwrap().into_inner();

                if !readbuf.is_empty() {
                    tracing::debug!("Inserting additional indirection layer due to remaining bytes in the read buffer");
                    r = Box::pin(websocat_api::util::PrependReader(readbuf, r));
                }

                Ok(websocat_api::Bipipe {
                    r: websocat_api::Source::ByteStream(r),
                    w: websocat_api::Sink::ByteStream(w),
                    closing_notification: cn,
                })
            } else {
                let body = resp.into_body();

                let r = websocat_api::Source::Datagrams(Box::pin(body.map_err(|e| e.into())));

                //let (r,w) = io.unwrap().into_inner();
                Ok(websocat_api::Bipipe {
                    r,
                    w: websocat_api::Sink::None,
                    closing_notification: cn,
                })
            }
        }
    }
}
