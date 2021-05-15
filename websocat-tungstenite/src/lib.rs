#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "wsc",
    prefix="wsc",
)]
pub struct WebsocketClient {
}

use websocat_api::bytes;
use tokio_tungstenite::tungstenite::Message;

#[websocat_api::async_trait::async_trait]
impl websocat_api::Node for WebsocketClient {
    #[tracing::instrument(level="debug", name="WebsocketClient", err, skip(_q, _w))]
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _q: websocat_api::RunContext, _w: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        use futures::stream::{StreamExt,TryStreamExt};
        use futures::sink::SinkExt;

        let (wss, _resp) = tokio_tungstenite::connect_async("ws://echo.websocket.org/").await?;
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
            closing_notification: None,
        })
    }
}
