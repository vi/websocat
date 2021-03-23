use websocat_api::{anyhow, bytes, futures, tokio};
use websocat_api::{
    async_trait::async_trait, Bipipe, Node, NodeId, Result, RunContext, Sink, Source,
};
use websocat_derive::{WebsocatEnum, WebsocatNode};

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "identity")]
pub struct Identity {
    /// inner node to be identical to
    inner: NodeId,
}

#[async_trait]
impl Node for Identity {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        tracing::debug!("Before running inner node of identity node");
        let x = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;
        tracing::debug!("After running inner node of identity node");
        Ok(x)
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, WebsocatEnum)]
#[websocat_enum(rename_all_lowercase)]
pub enum NodeMode {
    Bytes,
    Datagrams,
}

fn validate_buffer_size(bs: &mut Option<i64>, def: i64) -> Result<()> {
    if bs.is_none() {
        *bs = Some(def);
    }
    if bs.unwrap() < 1 {
        anyhow::bail!("buffer_size must be positive");
    }
    if bs.unwrap() > 100 * 1024 * 1024 {
        tracing::warn!("Suspiciously large buffer size encountered");
    }
    Ok(())
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "mirror")]
pub struct Mirror {
    /// bytestream mirror of datagram mirror
    #[websocat_prop(enum)]
    pub mode: NodeMode,

    /// Size of the buffer in bytes mode
    #[websocat_prop(default=1024, min=1, reasonable_max=100_000_000)]
    pub buffer_size: i64,
}

#[async_trait]
impl Node for Mirror {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        match self.mode {
            NodeMode::Bytes => {
                let (tx, rx) = tokio::io::duplex(self.buffer_size as usize);
                Ok(Bipipe {
                    r: Source::ByteStream(Box::pin(rx)),
                    w: Sink::ByteStream(Box::pin(tx)),
                    closing_notification: None,
                })
            }
            NodeMode::Datagrams => {
                let (tx, rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
                let tx2 = futures::sink::unfold(tx, move |tx, buf: bytes::Bytes| async move {
                    tracing::trace!("{} bytes buffer goes into the mirror", buf.len());
                    tx.send(buf).await?;
                    Ok(tx)
                });
                let rx2 = futures::stream::unfold(rx, move |mut rx| async move {
                    let buf = rx.recv().await;
                    buf.map(move |x| (Ok(x), rx))
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

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "devnull")]
pub struct DevNull {
    /// bytestream void of datagram void
    #[websocat_prop(enum)]
    pub mode: NodeMode,
}

#[async_trait]
impl Node for DevNull {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        match self.mode {
            NodeMode::Bytes => Ok(Bipipe {
                r: Source::ByteStream(Box::pin(tokio::io::empty())),
                w: Sink::ByteStream(Box::pin(tokio::io::sink())),
                closing_notification: None,
            }),
            NodeMode::Datagrams => {
                use futures::sink::SinkExt;
                Ok(Bipipe {
                    r: Source::Datagrams(Box::pin(futures::stream::empty())),
                    w: Sink::Datagrams(Box::pin(futures::sink::drain().sink_map_err(|x| x.into()))),
                    closing_notification: None,
                })
            }
        }
    }
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "split")]
pub struct Split {
    /// Subnode to use for receiving data
    pub r: Option<NodeId>,

    /// Subnode to use for sending data
    pub w: Option<NodeId>,
}

#[async_trait]
impl Node for Split {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        match (self.r, self.w) {
            (None, None) => {
                tracing::info!("Split node is fully dummy, neither `rx` nor `tx`.");
                Ok(Bipipe {
                    r: Source::None,
                    w: Sink::None,
                    closing_notification: None,
                })
            }
            (Some(rx), None) => {
                let x = ctx.nodes[rx].clone().run(ctx, None).await?;
                tracing::info!("Split node is removing the writing part from inner node.");
                Ok(Bipipe {
                    r: x.r,
                    w: Sink::None,
                    closing_notification: x.closing_notification,
                })
            }
            (None, Some(tx)) => {
                let x = ctx.nodes[tx].clone().run(ctx, None).await?;
                tracing::info!("Split node is removing the reading part from inner node.");
                Ok(Bipipe {
                    r: Source::None,
                    w: x.w,
                    closing_notification: x.closing_notification,
                })
            }
            (Some(rx), Some(tx)) => {
                let mut xr: Option<Bipipe> = None;
                let mut xw: Option<Bipipe> = None;
                let rn = ctx.nodes[rx].clone();
                let wn = ctx.nodes[tx].clone();
                let mut rxf = rn.run(ctx.clone(), None);
                let mut txf = wn.run(ctx, None);
                while xr.is_none() || xw.is_none() {
                    tokio::select! {
                        rr = &mut rxf, if xr.is_none() => {
                            xr = Some(rr?);
                            tracing::debug!("split nodes's reading part finished initializing first");
                        },
                        ww = &mut txf, if xw.is_none() => {
                            xw = Some(ww?);
                            tracing::debug!("split nodes's writing part finished initializing first");
                        },
                    }
                }
                let xr = xr.unwrap();
                let xw = xw.unwrap();
                let cn = match (xr.closing_notification, xw.closing_notification) {
                    (None, None) => None,
                    (Some(t), None) => Some(t),
                    (None, Some(t)) => Some(t),
                    (Some(_tr), Some(tw)) => {
                        tracing::debug!("split node is preferring to preserve write part's closing notification over reading part's one");
                        Some(tw)
                    }
                };
                tracing::debug!("split node is ready");
                Ok(Bipipe {
                    r: xr.r,
                    w: xw.w,
                    closing_notification: cn,
                })
            }
        }
    }
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "literal")]
pub struct Literal {
    /// List of explicit datagrams to provide as a datagram source
    pub bufs: Vec<bytes::Bytes>,
}

#[async_trait]
impl Node for Literal {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        use futures::stream::StreamExt;
        let src = futures::stream::iter(self.bufs.clone()).map(|x| Ok(x));
        Ok(Bipipe {
            r: Source::Datagrams(Box::pin(src)),
            w: Sink::None,
            closing_notification: None,
        })
    }
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "stream", validate)]
pub struct Stream {
    /// The node whose datagram sequences are to be converted to bytestreams
    pub inner: NodeId,

    /// Buffer size for temporary reading area
    pub buffer_size_r: Option<i64>,

    /// Buffer size for temporary writing area
    pub buffer_size_w: Option<i64>,
}

impl Stream {
    fn validate(&mut self) -> Result<()> {
        validate_buffer_size(&mut self.buffer_size_r, 1024)?;
        validate_buffer_size(&mut self.buffer_size_w, 1024)?;
        Ok(())
    }
}

#[async_trait]
impl Node for Stream {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        let p = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;

        if !matches!(p.r, Source::Datagrams(_)) && !matches!(p.w, Sink::Datagrams(_)) {
            tracing::warn!("Redundant use of `bytestream` node");
        }

        let r: Source = match p.r {
            Source::Datagrams(dgs) => {
                let (tx, rx) = tokio::io::duplex(self.buffer_size_r.unwrap() as usize);
                use futures::{StreamExt, TryStreamExt};
                use tokio_util::codec::BytesCodec;
                let dgs = dgs.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
                let w = tokio_util::codec::FramedWrite::new(tx, BytesCodec::new());

                tokio::spawn(async move {
                    if let Err(e) = dgs.forward(w).await {
                        tracing::error!("Error from `stream` node's read part: {}", e);
                    }
                });
                Source::ByteStream(Box::pin(rx))
            }
            x => x,
        };

        let w: Sink = match p.w {
            Sink::Datagrams(dgs) => {
                let (tx, rx) = tokio::io::duplex(self.buffer_size_w.unwrap() as usize);
                use futures::StreamExt;
                use tokio_util::codec::BytesCodec;
                let r = tokio_util::codec::FramedRead::new(rx, BytesCodec::new());

                tokio::spawn(async move {
                    if let Err(e) = r
                        .map(|x| x.map(|y| y.freeze()).map_err(|e| e.into()))
                        .forward(dgs)
                        .await
                    {
                        tracing::error!("Error from `stream` node's write part: {}", e);
                    }
                });
                Sink::ByteStream(Box::pin(tx))
            }
            x => x,
        };

        Ok(Bipipe {
            r,
            w,
            closing_notification: p.closing_notification,
        })
    }
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "datagrams")]
pub struct Datagrams {
    /// The node whose datagram sequences are to be converted to bytestreams
    pub inner: NodeId,
}

#[async_trait]
impl Node for Datagrams {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        let p = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;

        if !matches!(p.r, Source::ByteStream(_)) && !matches!(p.w, Sink::ByteStream(_)) {
            tracing::warn!("Redundant use of `datagrams` node");
        }

        let r: Source = match p.r {
            Source::ByteStream(s) => {
                use futures::{StreamExt, TryStreamExt};
                use tokio_util::codec::BytesCodec;
                let r = tokio_util::codec::FramedRead::new(s, BytesCodec::new());
                let r = r.map_err(|e| e.into());
                let r = r.map(|x| x.map(|y| y.freeze()));
                Source::Datagrams(Box::pin(r))
            }
            x => x,
        };

        let w: Sink = match p.w {
            Sink::ByteStream(s) => {
                use tokio_util::codec::BytesCodec;
                use futures::SinkExt;
                let w = tokio_util::codec::FramedWrite::new(s, BytesCodec::new());
                let w = w.sink_map_err(|e|e.into());
                Sink::Datagrams(Box::pin(w))
            }
            x => x,
        };

        Ok(Bipipe {
            r,
            w,
            closing_notification: p.closing_notification,
        })
    }
}

pub mod reuse;
