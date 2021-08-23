#![allow(unused)]
use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result, http,
};
use websocat_derive::WebsocatNode;
#[derive(Debug, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-server", validate)]
#[derivative(Clone)]
pub struct HttpServer {
    /// IO bytestream node to use
    inner: NodeId,
}

impl HttpServer {
    fn validate(&mut self) -> Result<()> {
        Ok(())
    }
}

async fn handle_request(rq : hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>> {
    tracing::info!("rq: {:?}", rq);
    Ok(hyper::Response::new(hyper::Body::empty()))
}

#[async_trait]
impl websocat_api::RunnableNode for HttpServer {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: websocat_api::RunContext, multiconn: Option<websocat_api::ServerModeContext>) -> Result<websocat_api::Bipipe> {
        let mut io = None;
        let mut cn = None;

        let io_ = ctx.nodes[self.inner].clone().upgrade()?.run(ctx.clone(), multiconn).await?;
        cn = io_.closing_notification;
        io = Some(match (io_.r, io_.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("HTTP server requires a bytestream-based inner node");
            }
        });

        let http = hyper::server::conn::Http::new();
        let q = http.serve_connection(io.unwrap(), hyper::service::service_fn(handle_request));

        use websocat_api::futures::TryFutureExt;
        tokio::spawn(q.map_err(|e|{
            tracing::error!("hyper server error: {}", e);
            ()
        }));

        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::None,
            w: websocat_api::Sink::None,
            closing_notification: cn,
        })
    }
}
