#![allow(unused_imports)]
use websocat_api::{anyhow, bytes, futures, tokio};
use websocat_api::{
    async_trait::async_trait, Bipipe, Node, NodeId, Result, RunContext, Sink, Source,
};
use websocat_derive::{WebsocatEnum, WebsocatNode};

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "spawner")]
pub struct Spawner {
    /// The node which should be recreated each time a new datagram comes
    pub inner: NodeId,

    /// Do not expect any replies from the inner node
    #[websocat_prop(default=false)]
    pub no_replies: bool,

    /// Number of requests before forced node reconnection
    #[websocat_prop(default=1, min=1)]
    pub n_requests: i64,

    /// Drop subnode writer early when no more requests to be sent
    #[websocat_prop(default=false)]
    pub early_drop: bool,
}

#[async_trait]
impl Node for Spawner {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        _multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        let (tx_rpl,rx_rpl) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);

        let r = if ! self.no_replies {
            let stream = futures::stream::unfold(rx_rpl, move |mut rx| {
                async move {
                    loop {
                        match rx.recv().await {
                            Some(buf) => {
                                tracing::trace!("Incoming buffer from spawner, {} bytes", buf.len());
                                return Some(( Ok(buf) , rx))
                            }
                            None => {
                                tracing::debug!("swapner's replies channel is closed, so our stream is ended too");
                                return None;
                            }
                        }
                    }
                    
                }
            });
            Source::Datagrams(Box::pin(stream))
        } else {
            Source::None
        };

        struct SinkState {
            dgrsink: Option<websocat_api::DatagramSink>,
            remaining_requests: usize,
        }

        let initial = SinkState {
            dgrsink: None,
            remaining_requests: 0,
        };
        let w = {
            let sink = futures::sink::unfold(initial, move |mystate, buf| {
                let tx_rpl = tx_rpl.clone();
                let ctx = ctx.clone();
                let inner = self.inner;
                let no_replies = self.no_replies;
                let n_requests = self.n_requests;
                let early_drop = self.early_drop;
                async move {
                    let (mut dgrsink, mut remaining_requests) = match mystate {
                        SinkState{dgrsink:Some(x), remaining_requests} if remaining_requests > 0 => {
                            tracing::debug!("Reusing spawner's subnode");
                            (x,remaining_requests)
                        }
                        _ => {
                            tracing::debug!("Spawner is creating a new subnode");
                            let p = ctx.nodes[inner].clone().run(ctx, None).await?;
                            drop(p.closing_notification);
                            let dgrsink = match p.w {
                                Sink::ByteStream(_) => anyhow::bail!("spawner supports only datagram-based nodes. Wrap your inner not in some adapter."),
                                Sink::Datagrams(x) => x,
                                Sink::None => anyhow::bail!("spawner is not meaningful for unwriteable nodes."),
                            };
                            let dgrsrc = match (p.r, no_replies) {
                                (Source::ByteStream(_), _) => anyhow::bail!("spawner supports only datagram-based nodes. Wrap your inner not in some adapter."),
                                (Source::None, true) => None,
                                (Source::None, false) => {
                                    tracing::warn!("spawner's inner node is write-only. Specify `n_replies` option to `0` to ignore this warning.");
                                    None
                                }
                                (Source::Datagrams(_), true) => {
                                    tracing::debug!("spawner's inner node is not write-only, but we have `no_replies` option, so ignoring the replies stream.");
                                    None
                                }
                                (Source::Datagrams(x), false) => Some(x),
                            };
        
                            if let Some(mut dgsrc) = dgrsrc {
                                tokio::spawn(async move {
                                    use futures::stream::StreamExt;
                                    loop {
                                        match dgsrc.next().await {
                                            Some(Ok(buf)) => {
                                                match tx_rpl.send(buf).await {
                                                    Ok(()) => {
                                                        tracing::trace!("Sent a buffer from spawner's subnode into outer node");
                                                    }
                                                    Err(_e) => {
                                                        tracing::debug!("Failed to send a buffer from spawner's subnode to outer node; exiting task");
                                                        break;
                                                    }
                                                }
                                            }
                                            Some(Err(e)) => {
                                                tracing::error!("Error reading from spawner's subnode: {}", e);
                                            }
                                            None => {
                                                tracing::debug!("spawner finished reading from subnode");
                                                break;
                                            }
                                        }
                                    }
                                });
                            }

                            (dgrsink, n_requests as usize)
                        }
                    };

                    use futures::sink::SinkExt;
                    dgrsink.send(buf).await?;

                    remaining_requests -= 1;

                    if early_drop && remaining_requests == 0 {
                        tracing::debug!("Immediately dropping inner subnode due to `eary_drop` mode activated");
                        Ok((SinkState {
                            dgrsink: None,
                            remaining_requests: 0,
    
                        }))
                    } else {
                        tracing::debug!("{} requests remaining to be served by inner subnode of spawner", remaining_requests);
                        Ok((SinkState {
                            dgrsink: Some(dgrsink),
                            remaining_requests: remaining_requests,
    
                        }))
                    }
                }
            });
            Sink::Datagrams(Box::pin(sink))
        };

        Ok(Bipipe {
            r,
            w,
            closing_notification: None,
        })

        //let p = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;
        //todo!()
    }
}
