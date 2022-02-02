
#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "stdio",
    prefix="stdio",
)]
#[auto_populate_in_allclasslist]
pub struct Stdio {
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for Stdio {
    #[tracing::instrument(level="debug", name="Stdio", err, skip(_q, _w))]
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _q: websocat_api::RunContext, _w: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        tracing::trace!("Obtaining stdin and stdout");
        let r = tokio::io::stdin();
        let w = tokio::io::stdout();
        tracing::debug!("Obtained stdin and stdout");
        Ok(websocat_api::Bipipe {
            r : websocat_api::Source::ByteStream(Box::pin(r)),
            w : websocat_api::Sink::ByteStream(Box::pin(w)),
            closing_notification: None,
        })
    }
}
