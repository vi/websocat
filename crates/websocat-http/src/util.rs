use websocat_api::{bytes, anyhow};

pub(crate) fn body_sink(sender: hyper::body::Sender) -> impl futures::sink::Sink<bytes::Bytes, Error=anyhow::Error> {
    let sink = futures::sink::unfold(
        sender,
        move |mut sender, buf: bytes::Bytes| async move {
            tracing::trace!("Sending {} bytes chunk to HTTP body", buf.len());
            sender.send_data(buf).await.map_err(|e| {
                tracing::error!("Failed sending more to HTTP body: {}", e);
                e
            })?;
            Ok(sender)
        },
    );
    sink
}

pub(crate) fn body_source(body_response_rx: tokio::sync::mpsc::Receiver<bytes::Bytes>) -> impl futures::stream::Stream<Item=anyhow::Result<bytes::Bytes>> {
    let rx = futures::stream::unfold(body_response_rx, move |mut response_rx| async move {
        let maybe_buf: Option<bytes::Bytes> = response_rx.recv().await;
        if maybe_buf.is_none() {
            tracing::debug!("HTTP body source finished");
        }
        maybe_buf.map(move |buf| {
            tracing::trace!("Accepted {} bytes from HTTP body", buf.len());
            (Ok(buf), response_rx)
        })
    });
    rx
}
