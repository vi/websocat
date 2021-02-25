
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
impl websocat_api::ParsedNode for Tcp {
    async fn run(&self, _: websocat_api::RunContext, _: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        let c = tokio::net::TcpStream::connect(self.addr).await?;
        let (r,w) = c.into_split();
        Ok(websocat_api::Pipe {
            r : Box::pin(r),
            w : Box::pin(w),
            closing_notification: None,
        })
    }
}
