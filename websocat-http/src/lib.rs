use std::any;

use websocat_api::{anyhow, bytes, tokio, async_trait::async_trait};
use websocat_derive::WebsocatNode;

#[derive(Debug,Clone,WebsocatNode)]
#[websocat_node(
    official_name = "http-client"
)]
pub struct HttpClient {
    /// IO object to use for HTTP1 handshake
    inner: websocat_api::NodeId,
}

struct DummyBody;

impl hyper::body::HttpBody for DummyBody {
    type Data = bytes::Bytes;
    type Error = anyhow::Error;

    fn poll_data(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Self::Data, Self::Error>>> {
        std::task::Poll::Ready(None)
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<hyper::HeaderMap>, Self::Error>> {
        std::task::Poll::Ready(Ok(None))
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

        let b = hyper::client::conn::Builder::new().handshake::<_,DummyBody>(io.take().unwrap());
        let (mut sr, conn) = b.await?;
        let h = tokio::spawn(conn/* .without_shutdown() */);

        let rq = hyper::Request::new(DummyBody);

        let resp = sr.send_request(rq).await?;

        let upg = hyper::upgrade::on(resp).await?;

        //let parts = h.await??;
        //parts.read_buf // TODO
        //let io = parts.io;

        io = Some(upg.downcast().unwrap().io);
    

        let (r,w) = io.unwrap().into_inner();
        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::ByteStream(r),
            w: websocat_api::Sink::ByteStream(w),
            closing_notification: cn,
        })
    }
}