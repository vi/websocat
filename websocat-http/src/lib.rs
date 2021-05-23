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
#[websocat_node(official_name = "header", data_only)]
pub struct Header {
    /// HTTP header name
    n: String,
    /// HTTP header value
    v: String,
}

