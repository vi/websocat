use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "http-client", validate)]
pub struct HttpClient {
    /// IO object to use for HTTP1 handshake
    inner: NodeId,

    /// Expect and work upon upgrades
    upgrade: Option<bool>,

    /// Immediately return connection, stream bytes into request body.
    stream_request_body: Option<bool>,

    /// Subnode to read request body from
    request_body: Option<NodeId>,

    /// Fully read request body into memory prior to sending it
    buffer_request_body: Option<bool>,

    /// Preallocate this amount of memory for caching request body
    buffer_request_body_size_hint: Option<i64>,
}

impl HttpClient {
    fn validate(&mut self) -> Result<()> {
        if self.stream_request_body == Some(true) {
            if self.upgrade == Some(true) {
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

        if self.buffer_request_body == Some(true)
            && (self.request_body.is_none() && self.stream_request_body != Some(true))
        {
            anyhow::bail!("buffer_request_body option is meaningless withouth stream_request_body or request_body options");
        }

        if self.buffer_request_body != Some(true) && self.buffer_request_body_size_hint.is_some() {
            anyhow::bail!("buffer_request_body_size_hint option is meaningless withouth buffer_request_body option");
        }

        if let Some(sz) = self.buffer_request_body_size_hint {
            if sz < 0 {
                anyhow::bail!("buffer_request_body_size_hint option should have nonnegative value");
            }
            if sz > 1024 * 1024 * 100 {
                tracing::warn!("buffer_request_body_size_hint have suspicously large value");
            }
        }

        if self.buffer_request_body == Some(true) && self.buffer_request_body_size_hint.is_none() {
            self.buffer_request_body_size_hint = Some(1024);
        }

        Ok(())
    }
}

impl HttpClient {
    fn get_request(&self, body: hyper::Body) -> Result<hyper::Request<hyper::Body>> {
        let mut rq = hyper::Request::new(body);
        *rq.method_mut() = hyper::Method::POST;
        //rq.headers_mut().insert(hyper::header::CONTENT_TYPE, "text/plain".parse().unwrap());
        //rq.headers_mut().insert(hyper::header::TRANSFER_ENCODING, "identity".parse().unwrap());
        //rq.headers_mut().insert(hyper::header::CONTENT_LENGTH, "15".parse().unwrap());
        Ok(rq)
    }
}

#[async_trait]
impl websocat_api::Node for HttpClient {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: websocat_api::RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {
        let io = ctx.nodes[self.inner].clone().run(ctx.clone(), None).await?;
        let cn = io.closing_notification;
        let mut io = Some(match (io.r, io.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("HTTP client requires bytestream-based inner node");
            }
        });

        let b = hyper::client::conn::Builder::new().handshake::<_, hyper::Body>(io.take().unwrap());
        let (mut sr, conn) = b.await?;
        let _h = tokio::spawn(conn /* .without_shutdown() */);

        if self.stream_request_body == Some(true) {
            let (response_tx, response_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
            let w: websocat_api::Sink = if self.buffer_request_body != Some(true) {
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
                        //let mut rq = self.get_request(request_body)?;
                        let rq = hyper::Request::new(request_body);
                        let resp = sr.send_request(rq).await?;
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
                    self.buffer_request_body_size_hint.unwrap() as usize,
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
                        //let mut rq = self.get_request(request_buf.freeze().into())?;
                        let rq = hyper::Request::new(request_buf.freeze().into());
                        let resp = sr.send_request(rq).await?;
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
            let rq = hyper::Request::new(hyper::Body::empty());

            let resp = sr.send_request(rq).await?;

            if self.upgrade == Some(true) {
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
