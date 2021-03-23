#![allow(unused)]
use websocat_api::{anyhow, bytes, futures, tokio};
use websocat_api::{
    async_trait::async_trait, Bipipe, Node, NodeId, Result, RunContext, Sink, Source,
};
use websocat_derive::{WebsocatEnum, WebsocatNode};


#[derive(Debug)]
enum ReuserDataChoice {
    Broadcast(tokio::sync::broadcast::Receiver<bytes::Bytes>),
}

#[derive(Debug)]
struct ReuserData {
    tx: tokio::sync::mpsc::Sender<bytes::Bytes>,
    rx: ReuserDataChoice,
}

#[derive(Debug, WebsocatNode)]
#[websocat_node(official_name = "reuse")]
pub struct Reuse {
    /// The node, whose connection is kept persistent and is reused when `reuse` node is reinvoked
    pub inner: NodeId,

    /// Whether to route information coming from the inner node to all connecting nodes or to just some one of them
    pub broadcast: bool,

    /// How many concurrent users to allow
    pub simultaneous_user_limit: Option<i64>,

    #[websocat_prop(ignore)]
    the_pipe : tokio::sync::Mutex<Option<ReuserData>>,
}

impl Clone for Reuse {
    fn clone(&self) -> Self {
        Reuse {
            inner: self.inner,
            broadcast: self.broadcast,
            simultaneous_user_limit: self.simultaneous_user_limit,
            the_pipe: Default::default(),
        }
    }
}

#[async_trait]
impl Node for Reuse {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        let p = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;
        tracing::error!("`reuse` is not implemented");
        Ok(Bipipe {
            r: p.r,
            w: p.w,
            closing_notification: p.closing_notification,
        })
    }
}
