use super::{RunContext,IWantToServeAnotherConnection, NodeProperyAccess, Result};

pub enum Source {
    ByteStream(Box<dyn std::io::Read + Send + 'static>),
    Datagrams(Box<dyn FnMut()->Result<bytes::BytesMut> + Send + 'static>),
}

pub enum Sink {
    ByteStream(Box<dyn std::io::Write + Send + 'static>),
    Datagrams(Box<dyn FnMut(bytes::BytesMut)->Result<()> + Send + 'static>),
}

pub struct Bipipe {
    pub r: Source,
    pub w: Sink,
    pub closing_notification: Option<tokio::sync::oneshot::Receiver<()>>,
}
pub trait Node: NodeProperyAccess {
    /// Started from a Tokio runtime thread, so don't block it, spawn your own thread to handle things.
    /// If this is a server that does multiple connections, start `closure` in a loop.
    fn run(&self, ctx: RunContext, allow_multiconnect: bool, closure: impl FnMut(Bipipe) -> Result<()> + Send ) -> Result<()>;
}

#[async_trait::async_trait]
impl<T:Node + Send + Sync + 'static> super::Node for T {
    async fn run(&self, _ctx: RunContext, _multiconn: &mut IWantToServeAnotherConnection) -> Result<super::Bipipe> {
        Err(anyhow::anyhow!("nimpl"))
    }
}

