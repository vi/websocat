
use websocat_api::bytes;
use tokio_tungstenite::tungstenite::Message;
use websocat_api::http::Uri;
use websocat_api::NodeId;
use websocat_api::anyhow;
use websocat_api::ClosingNotification;
use websocat_api::tokio::io::{AsyncRead,AsyncWrite};

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "wsc",
    prefix="wsc",
)]
#[auto_populate_in_allclasslist]
pub struct WebsocketClient {
    /// URL that specifies where to connect to
    uri: Uri,

    /// Underlying node to use for raw I/O instead of usual TCP or TLS connection 
    inner: Option<NodeId>,
}



#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for WebsocketClient {
    #[tracing::instrument(level="debug", name="WebsocketClient", err, skip(ctx, multiconn))]
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: websocat_api::RunContext, multiconn: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        let mut closing_notification = None;

        if let Some(inn) = self.inner {
            let io = ctx.nodes[inn].clone().upgrade()?.run(ctx, multiconn).await?;
            closing_notification = io.closing_notification;
            let io = match (io.r, io.w) {
                (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                    readwrite::ReadWriteTokio::new(r, w)
                }
                _ => {
                    anyhow::bail!("Websocket requires bytestream-based inner node");
                }
            };
            let (wss, _resp) = tokio_tungstenite::client_async(&self.uri, io).await?;
            wss_to_node(wss, closing_notification)
        } else { 
            let (wss, _resp) = tokio_tungstenite::connect_async(&self.uri).await?;
            wss_to_node(wss, closing_notification)
        }
    }
}


#[tracing::instrument(level="debug", name="wss_to_node", err, skip(wss, closing_notification))]
fn wss_to_node<T>(wss: tokio_tungstenite::WebSocketStream<T> , closing_notification: Option<ClosingNotification> ) 
-> websocat_api::Result<websocat_api::Bipipe>
where T: AsyncRead + AsyncWrite + Unpin + Send + 'static
 {
    use futures::stream::{StreamExt,TryStreamExt};
    use futures::sink::SinkExt;
    let (wss1,wss2) = wss.split();
        
    let wss1 = wss1.with(|buf : bytes::Bytes|{
        async move {
            Ok(Message::Binary(buf.to_vec()))
        }
    });

    let wss2 = wss2.try_filter_map(|msg| {
        async move {
            match msg {
                Message::Text(x) => Ok(Some(x.into())),
                Message::Binary(x) => Ok(Some(x.into())),
                Message::Ping(_) => Ok(None),
                Message::Pong(_) => Ok(None),
                Message::Close(_) => Ok(None),
            }
        }
    }).map_err(|e|e.into());
    Ok(websocat_api::Bipipe {
        r : websocat_api::Source::Datagrams(Box::pin(wss2)),
        w : websocat_api::Sink::Datagrams(Box::pin(wss1)),
        closing_notification,
    })
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, websocat_derive::WebsocatEnum)]
#[websocat_enum(rename_all_lowercase)]
pub enum WebsocketLlRole {
    Client,
    Server,
}

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "wsll",
    prefix="wsll",
)]
#[auto_populate_in_allclasslist]
pub struct WebsocketLowlevel {
    /// Underlying node to use for raw I/O instead of usual TCP or TLS connection 
    inner: NodeId,

    /// Whether it is WebSocket client or server
    #[websocat_prop(enum)]
    role: WebsocketLlRole,
}


#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for WebsocketLowlevel {
    #[tracing::instrument(level="debug", name="WebsocketLowlevel", err, skip(ctx, multiconn))]
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, ctx: websocat_api::RunContext, multiconn: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        let io = ctx.nodes[self.inner].clone().upgrade()?.run(ctx, multiconn).await?;
        let closing_notification = io.closing_notification;
        let io = match (io.r, io.w) {
            (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                readwrite::ReadWriteTokio::new(r, w)
            }
            _ => {
                anyhow::bail!("Websocket requires bytestream-based inner node");
            }
        };
        let role = match self.role {
            WebsocketLlRole::Client => tokio_tungstenite::tungstenite::protocol::Role::Client,
            WebsocketLlRole::Server => tokio_tungstenite::tungstenite::protocol::Role::Server,
        };

        let wss = tokio_tungstenite::WebSocketStream::from_raw_socket(io, role, None).await;
        wss_to_node(wss, closing_notification)
    }
}
