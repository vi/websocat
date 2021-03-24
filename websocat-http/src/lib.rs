#[allow(unused_imports)]
use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;

pub mod lowlevel_client;
pub use lowlevel_client::HttpClient;

pub mod highlevel_client;
pub use highlevel_client::HttpClient as HttpHighlevelClient;

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "header")]
pub struct Header {
    /// HTTP header name
    n: String,
    /// HTTP header value
    v: String,
}

#[async_trait]
impl websocat_api::Node for Header {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _ctx: websocat_api::RunContext, _multiconn: Option<websocat_api::ServerModeContext>) -> Result<websocat_api::Bipipe> {
        anyhow::bail!("`header` nodes are not supposed to be used directly, only as array elements of http-client or http-server nodes")
    }
}


