use websocat_api::{anyhow, tokio, futures, bytes};
use websocat_api::{Bipipe, Node, RunContext, Result, NodeId, async_trait::async_trait, Source, Sink};
use websocat_derive::{WebsocatNode, WebsocatEnum};

#[derive(Debug,Clone,WebsocatNode)]
#[websocat_node(
    official_name="identity"
)]
pub struct Identity {
    /// inner node to be identical to
    inner : NodeId,
}

#[async_trait]
impl Node for Identity {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: RunContext, multiconn: Option<websocat_api::ServerModeContext>) -> Result<Bipipe> {
        tracing::debug!("Before running inner node of identity node");
        let x = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;
        tracing::debug!("After running inner node of identity node");
        Ok(x)
    }
}

#[derive(Debug,Eq,PartialEq,Copy,Clone,WebsocatEnum)]
#[websocat_enum(rename_all_lowercase)]
pub enum NodeMode {
    Bytes,
    Datagrams,
}

#[derive(Debug,Clone,WebsocatNode)]
#[websocat_node(
    official_name="mirror",
    validate,
)]
pub struct Mirror {
    /// bytestream mirror of datagram mirror
    #[websocat_node(enum)]
    pub mode : NodeMode,

    /// Size of the buffer in bytes mode
    pub buffer_size: Option<i64>,
}

impl Mirror {
    fn validate(&mut self) -> Result<()> {
        match self.mode {
            NodeMode::Datagrams => {
                if self.buffer_size.is_some() {
                    anyhow::bail!("Settign buffer_size in datagrams mode is meaningless");
                }
            }
            NodeMode::Bytes => {
                if self.buffer_size.is_none() {
                    self.buffer_size = Some(1024);
                }
                if self.buffer_size.unwrap() < 1 {
                    anyhow::bail!("buffer_size must be positive");
                }
                if self.buffer_size.unwrap() > 100*1024*1024 {
                    tracing::warn!("Suspiciously large buffer_size in mirror node");
                }
            }
        }
       
        Ok(())
    }
}

#[async_trait]
impl Node for Mirror {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _ctx: RunContext, _multiconn: Option<websocat_api::ServerModeContext>) -> Result<Bipipe> {
        match self.mode {
            NodeMode::Bytes => {
                let (tx,rx) = tokio::io::duplex(self.buffer_size.unwrap() as usize);
                Ok(Bipipe {
                    r: Source::ByteStream(Box::pin(rx)),
                    w: Sink::ByteStream(Box::pin(tx)),
                    closing_notification: None,
                })
            }
            NodeMode::Datagrams => {
                let (tx,rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
                let tx2 = futures::sink::unfold(tx, move |tx, buf: bytes::Bytes| {
                    async move {
                        tracing::trace!("{} bytes buffer goes into the mirror", buf.len());
                        tx.send(buf).await?;
                        Ok(tx)
                    }
                });
                let rx2 = futures::stream::unfold(rx, move |mut rx| {
                    async move {
                        let buf = rx.recv().await;
                        buf.map(move |x| (Ok(x), rx))
                    }
                });
                Ok(Bipipe {
                    r: Source::Datagrams(Box::pin(rx2)),
                    w: Sink::Datagrams(Box::pin(tx2)),
                    closing_notification: None,
                })
            }
        }
    }
}



#[derive(Debug,Clone,WebsocatNode)]
#[websocat_node(
    official_name="devnull",
)]
pub struct DevNull {
    /// bytestream void of datagram void
    #[websocat_node(enum)]
    pub mode : NodeMode,
}

#[async_trait]
impl Node for DevNull {
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _ctx: RunContext, _multiconn: Option<websocat_api::ServerModeContext>) -> Result<Bipipe> {
        match self.mode {
            NodeMode::Bytes => {
                Ok(Bipipe {
                    r: Source::ByteStream(Box::pin(tokio::io::empty())),
                    w: Sink::ByteStream(Box::pin(tokio::io::sink())),
                    closing_notification: None,
                })
            }
            NodeMode::Datagrams => {
                use futures::sink::SinkExt;
                Ok(Bipipe {
                    r: Source::Datagrams(Box::pin(futures::stream::empty())),
                    w: Sink::Datagrams(Box::pin(futures::sink::drain().sink_map_err(|x|x.into()))),
                    closing_notification: None,
                })
            }
        }
    }
}
