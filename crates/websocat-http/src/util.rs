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

/// Derive the `Sec-WebSocket-Accept` response header from a `Sec-WebSocket-Key` request header.
///
/// This function can be used to perform a handshake before passing a raw TCP stream to
/// [`WebSocket::from_raw_socket`][crate::protocol::WebSocket::from_raw_socket].
///
/// Based on https://github.com/snapview/tungstenite-rs/blob/985d6571923c2eac3310d8a9981a2306ae675214/src/handshake/mod.rs#L113
pub(crate) fn derive_websocket_accept_key(request_key: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::default();
    sha1.update(request_key);
    sha1.update(WS_GUID);
    base64::encode(&sha1.finalize())
}
