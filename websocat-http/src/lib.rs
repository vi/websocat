
use websocat_api::{anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, Result, NodeId};
use websocat_derive::WebsocatNode;

#[derive(Debug,Clone,WebsocatNode)]
#[websocat_node(
    official_name = "http-client",
    validate,
)]
pub struct HttpClient {
    /// IO object to use for HTTP1 handshake
    inner: NodeId,

    /// Expect and work upon upgrades
    upgrade: Option<bool>,

    /// Immediately return connection, stream bytes into request body.
    stream_request_body: Option<bool>,

    /// Subnode to read request body from
    body: Option<NodeId>,

    /// Tokio io channel buffer size when sending body in streamed mode
    stream_request_body_buffer_tx: Option<i64>,

    /// Tokio io channel buffer size when receining response in streamed mode
    stream_request_body_buffer_rx: Option<i64>,

}

impl HttpClient {
    fn validate(&mut self) -> Result<()> {
        if self.stream_request_body == Some(true) {
            if self.upgrade == Some(true) {
                anyhow::bail!("Cannot set both `upgrade` and `stream_request_body` options at the same time");
            } 
            if self.body.is_some() {
                anyhow::bail!("Cannot set both `body` and `stream_request_body` options at the same time");
            }
            if self.stream_request_body_buffer_rx.is_none() {
                self.stream_request_body_buffer_rx = Some(1024);
            }
            if self.stream_request_body_buffer_tx.is_none() {
                self.stream_request_body_buffer_tx = Some(1024);
            }
            if self.stream_request_body_buffer_rx.unwrap() < 1 {
                anyhow::bail!("stream_request_body_buffer_rx must be positive");
            }
            if self.stream_request_body_buffer_tx.unwrap() < 1 {
                anyhow::bail!("stream_request_body_buffer_tx must be positive");
            }
        } else {
            if self.stream_request_body_buffer_rx.is_some() {
                anyhow::bail!("stream_request_body_buffer_rx option is meaningless withouth stream_request_body");
            }
            if self.stream_request_body_buffer_tx.is_some() {
                anyhow::bail!("stream_request_body_buffer_tx option is meaningless withouth stream_request_body");
            }
        }
       
        Ok(())
    }
}

#[async_trait]
impl websocat_api::Node for HttpClient {
    async fn run(&self, ctx: websocat_api::RunContext, _multiconn: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        let io = ctx.nodes[self.inner].run(ctx.clone(), None).await?;
        let cn = io.closing_notification;
        let mut io = Some(match (io.r, io.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            },
            _ => {
                anyhow::bail!("HTTP client requires bytestream-based inner node");
            }
        });

        

        let b 
            = hyper::client::conn::Builder::new().handshake::<_,hyper::Body>(io.take().unwrap());
        let (mut sr, conn) = b.await?;
        let _h = tokio::spawn(conn/* .without_shutdown() */);

        if self.stream_request_body == Some(true) {
            let (sender, b) = hyper::Body::channel();
            let (response_tx, response_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
            tokio::spawn(async move {
                let try_block = async move {
                    let rq = hyper::Request::new(b);
                    let resp = sr.send_request(rq).await?;
                    let mut body = resp.into_body();
                    use futures::stream::StreamExt;
                    while let Some(buf) = body.next().await {
                        response_tx.send(buf?).await?;
                    }
                    Ok::<_,anyhow::Error>(())
                };
                if let Err(e) = try_block.await {
                    tracing::error!("streamed-http-client error: {}", e);
                }
            });
            let sink = futures::sink::unfold(sender, move |mut sender, buf| {
                async move {
                    sender.send_data(buf).await?;
                    Ok(sender)
                }
            });
            let rx = futures::stream::unfold(response_rx, move |mut response_rx| {
                async move {
                    let maybe_buf : Option<bytes::Bytes> = response_rx.recv().await;
                    maybe_buf.map(move |buf|((Ok(buf), response_rx)))
                }
            });
            Ok(websocat_api::Bipipe {
                r: websocat_api::Source::Datagrams(Box::pin(rx)),
                w: websocat_api::Sink::Datagrams(Box::pin(sink)),
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
            
        
                let (mut r,w) = io.unwrap().into_inner();
        
                if ! readbuf.is_empty() {
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
    
                let r = websocat_api::Source::Datagrams(Box::pin(body.map_err(|e|e.into())));
        
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
