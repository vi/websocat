#![allow(unused_imports)]
use websocat_api::{anyhow, bytes, futures, tokio};
use websocat_api::{
    async_trait::async_trait, Bipipe, Node, NodeId, Result, RunContext, Sink, Source,
};
use websocat_derive::{WebsocatEnum, WebsocatNode};


#[derive(Debug,Clone)]
struct ReuserData {
    tx: Option<tokio::sync::mpsc::Sender<bytes::Bytes>>,
    rx: Option<tokio::sync::broadcast::Sender<bytes::Bytes>>,
    cl: Option<tokio::sync::watch::Receiver<()>>,
}

#[derive(Debug, WebsocatNode)]
#[websocat_node(official_name = "reuse-broadcast")]
pub struct ReuseBroadcast {
    /// The node, whose connection is kept persistent and is reused when `reuse` node is reinvoked
    pub inner: NodeId,

    /// Buffer size for request datagrams, measured in packets (not bytes)
    /// if this buffer is filled up, clients' request sending capacity get slowed down
    #[websocat_prop(default=1, min=1, reasonable_max=1000_000)]
    buffer_requests: i64,

    /// Buffer size for broadcast reply datagrams, measured in packets (not bytes)
    /// if this buffer is filled up, reused nodes' replies get dropped for some clients
    #[websocat_prop(default=10, min=1, reasonable_max=1000_000)]
    buffer_replies: i64,

    /// Disconnect clients if they are too slow for receiving broadcast replies.
    /// Otherwise they get dropped instead
    #[websocat_prop(default=false)]
    disconnect_on_lag: bool,

    #[websocat_prop(ignore)]
    the_pipe : tokio::sync::Mutex<Option<ReuserData>>,
}

impl Clone for ReuseBroadcast {
    fn clone(&self) -> Self {
        ReuseBroadcast {
            inner: self.inner,
            the_pipe: Default::default(),
            buffer_requests: self.buffer_requests,
            buffer_replies: self.buffer_replies,
            disconnect_on_lag: self.disconnect_on_lag,
        }
    }
}

#[async_trait]
impl Node for ReuseBroadcast {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> Result<Bipipe> {
        let pip;
        {
            let mut l = self.the_pipe.lock().await;
            if l.is_none() {
                let p = ctx.nodes[self.inner].clone().run(ctx, multiconn).await?;

                let rpart = match p.r {
                    Source::ByteStream(_) => {anyhow::bail!("reuser works only on datagram-based data. Use `datagrams` node to convert.")}
                    Source::Datagrams(mut dsrc) => {
                        let (tx2, _rx2) = tokio::sync::broadcast::channel::<bytes::Bytes>(self.buffer_replies as usize);
                        
                        let tx2c = tx2.clone();
                        tokio::spawn(async move {
                            loop {
                                use futures::stream::StreamExt;
                                match dsrc.next().await {
                                    Some(Ok(buf)) => {
                                        match tx2c.send(buf) {
                                            Ok(n) => tracing::trace!("Broadcasted to {} clients", n),
                                            Err(_e) => tracing::warn!("No clients to broadcast to, dropping the message"),
                                        }
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("Error reading from reuse-broadcast'ed subnode: {}", e);
                                        break;
                                    }
                                    None => {
                                        tracing::debug!("Subnode of reuse-broadcast finished emitting its datagrams");
                                        break
                                    }
                                }
                            }
                        });
                        
                        Some(tx2)
                    }
                    Source::None => {None}
                };

                let wpart = match p.w {
                    Sink::ByteStream(_) => {anyhow::bail!("reuser works only on datagram-based data. Use `datagram` node to convert.")}
                    Sink::Datagrams(mut dsink) => {
                        let (tx1, mut rx1) = tokio::sync::mpsc::channel::<bytes::Bytes>(self.buffer_requests as usize);
                        
                        tokio::spawn(async move {
                            loop {
                                use futures::sink::SinkExt;
                                match rx1.recv().await {
                                    Some(buf) => {
                                        match dsink.send(buf).await {
                                            Ok(()) => {
                                                tracing::trace!("Sent a buffer to broadcast-reused subnode");
                                            }
                                            Err(e) => {
                                                tracing::error!("Cannot write into broadcast-reused subnode: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                    None => {
                                        tracing::debug!("No writing possible to this reuse-broadcast'ed submode. Exiting the writer task.");
                                        break;
                                    }
                                }
                            }
                        });
                        
                        Some(tx1)
                    }
                    Sink::None => {None}
                };

                *l = Some(
                    ReuserData {
                        tx: wpart,
                        rx: rpart,
                        cl: None, // TODO
                    }
                );
            } // end of initialisation part
            pip = l.as_ref().unwrap().clone();
        }

        let disconnect_on_lag = self.disconnect_on_lag;
        let r = match pip.rx {
            Some(rx) => {
                let stream = futures::stream::unfold(rx.subscribe(), move |mut rx| {
                    async move {
                        loop {
                            match rx.recv().await {
                                Ok(buf) => {
                                    tracing::trace!("Incoming buffer from reuse-broadcast, {} bytes", buf.len());
                                    return Some(( Ok(buf) , rx))
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    tracing::debug!("reuse-broadcast's channel is closed, so our stream is ended too");
                                    return None;
                                } 
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    if disconnect_on_lag {
                                        tracing::warn!("This client of reuse-broadcast is lagged, disconnecting it");
                                        return None;
                                    } else {
                                        tracing::warn!("This client of reuse-broadcast is lagged and has skipped {} messages", n);
                                        continue;
                                    }
                                }
                            }
                        }
                       
                    }
                });
                Source::Datagrams(Box::pin(stream))
            }
            None => Source::None,
        };
        let w = match pip.tx {
            Some(tx) => {
                let sink = futures::sink::unfold(tx, move |tx, buf| {
                    async move {
                        match tx.send(buf).await {
                            Ok(()) => {
                                tracing::trace!("Client of reuse-broadcast has sent a buffer");
                            }
                            Err(_e) => {
                                anyhow::bail!("Cannot send a message from reuse-broadcast's client to the reused subnode");
                            }
                        }
                        Ok((tx))
                    }
                });
                Sink::Datagrams(Box::pin(sink))
            }
            None => Sink::None,
        };
        

        Ok(Bipipe {
            r,
            w,
            closing_notification: None, // TODO
        })
    }
}
