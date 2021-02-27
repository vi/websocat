
#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "tcp",
    prefix="tcp",
)]
pub struct Tcp {
    /// Address where TCP-connect to
    addr : std::net::SocketAddr,
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::Node for Tcp {
    #[tracing::instrument(level="debug", name="Tcp", skip(self), err)]
    async fn run(&self, _: websocat_api::RunContext, _: Option<&mut websocat_api::IWantToServeAnotherConnection>) -> websocat_api::Result<websocat_api::Bipipe> {
        tracing::debug!("Connecting to {}", self.addr);
        let c = tokio::net::TcpStream::connect(self.addr).await?;
        let (r,w) = c.into_split();
        tracing::info!("Connected to {}", self.addr);
        Ok(websocat_api::Bipipe {
            r : websocat_api::Source::ByteStream(Box::pin(r)),
            w : websocat_api::Sink::ByteStream(Box::pin(w)),
            closing_notification: None,
        })
    }
}
