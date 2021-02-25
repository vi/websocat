
#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "stdio",
    prefix="stdio",
    debug_derive
)]
pub struct Stdio {
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::ParsedNode for Stdio {
    async fn run(&self, _: websocat_api::RunContext, _: &mut websocat_api::IWantToServeAnotherConnection) -> websocat_api::Result<websocat_api::Pipe> {
        let r = tokio::io::stdin();
        let w = tokio::io::stdout();
        Ok(websocat_api::Pipe {
            r : Box::pin(r),
            w : Box::pin(w),
            closing_notification: None,
        })
    }
}
