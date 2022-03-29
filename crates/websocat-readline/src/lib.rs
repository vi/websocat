use websocat_derive::{WebsocatNode};
use websocat_api::{Result, tracing};
use websocat_api::{Bipipe, RunnableNode, Source, Sink};
use websocat_api::{anyhow};


#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "readline")]
#[auto_populate_in_allclasslist]
pub struct Readline {
  
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for Readline {
    #[tracing::instrument(level="debug", name="Readline", err, skip(_q, _w))]
    async fn run(self: std::pin::Pin<std::sync::Arc<Self>>, _q: websocat_api::RunContext, _w: Option<websocat_api::ServerModeContext>) -> websocat_api::Result<websocat_api::Bipipe> {
        let (rl, wr) = rustyline_async::Readline::new("W% ".to_owned())?;
        tracing::debug!("Created rustyline_async");

        let sink = futures::sink::unfold(
            wr,
            move |mut wr, buf: websocat_api::bytes::Bytes| async move {
                tracing::trace!("Sending {} bytes chunk to rustyline_async", buf.len());
                use futures::io::AsyncWriteExt;
                wr.write_all(&buf).await?;
                Ok(wr)
            },
        );

        let rx = futures::stream::unfold(rl, move |mut rl| async move {
            loop {
                match rl.readline().await {
                    Some(Ok(x)) => {
                        tracing::debug!("Data from rustyline_async: `{}`", x);
                        return Some((Ok(x.into()), rl));
                    }
                    Some(Err(e)) => {
                        tracing::debug!("Error from rustyline_async: {}", e);
                        return Some((Err(e.into()), rl));
                    }
                    None => {
                        tracing::trace!("None from rustyline_async");
                        continue;
                    },
                }
            }
        });
        Ok(websocat_api::Bipipe {
            r : websocat_api::Source::Datagrams(Box::pin(rx)),
            w : websocat_api::Sink::Datagrams(Box::pin(sink)),
            closing_notification: None,
        })
    }
}

