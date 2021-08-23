use tokio::io::AsyncRead;


/// AsyncRead that first drains the supplied buffer, then continues to the inner node
#[pin_project::pin_project]
pub struct PrependReader<T>(pub bytes::Bytes, #[pin] pub T);

impl<T: AsyncRead> AsyncRead for PrependReader<T> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.0.is_empty() {
            AsyncRead::poll_read(self.project().1, cx, buf)
        } else {
            let b = self.project().0;
            let mut to_fill = buf.remaining();
            to_fill = to_fill.min(b.len());

            buf.put_slice(&b[..to_fill]);
            *b = b.slice(to_fill..);

            std::task::Poll::Ready(std::io::Result::Ok(()))
        }
    }
}