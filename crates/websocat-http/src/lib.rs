#[allow(unused_imports)]
use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result,
};
use websocat_derive::WebsocatNode;

pub mod client;
pub use client::{HttpClient,AutoLowlevelHttpClient};

pub mod client2;

pub mod server;
pub use server::HttpServer;

mod util;

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "header", data_only)]
#[auto_populate_in_allclasslist]
pub struct Header {
    /// HTTP header name
    n: String,
    /// HTTP header value
    v: String,
}


