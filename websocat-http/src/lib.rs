#[allow(unused_imports)]
use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;

pub mod client;
pub use client::HttpClient;

pub mod server;
pub use server::HttpServer;

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "header", data_only)]
pub struct Header {
    /// HTTP header name
    n: String,
    /// HTTP header value
    v: String,
}

