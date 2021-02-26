use std::pin::Pin;

use super::{IWantToServeAnotherConnection, NodeProperyAccess, Result, RunContext};

pub enum Source {
    ByteStream(Box<dyn std::io::Read + Send + 'static>),
    Datagrams(Box<dyn FnMut() -> Result<bytes::BytesMut> + Send + 'static>),
    None,
}

pub enum Sink {
    ByteStream(Box<dyn std::io::Write + Send + 'static>),
    Datagrams(Box<dyn FnMut(bytes::BytesMut) -> Result<()> + Send + 'static>),
    None,
}

pub struct Bipipe {
    pub r: Source,
    pub w: Sink,
    pub closing_notification: Option<tokio::sync::oneshot::Receiver<()>>,
}
pub trait Node: NodeProperyAccess {
    /// Started from a Tokio runtime thread, so don't block it, spawn your own thread to handle things.
    /// If this is a server that does multiple connections, start `closure` in a loop.
    /// The `closure` is supposed to run in a thread that can block
    fn run(
        &self,
        ctx: RunContext,
        allow_multiconnect: bool,
        closure: impl FnMut(Bipipe) -> Result<()> + Send + 'static,
    ) -> Result<()>;
}

struct SyncReadGateway {
    reqests: tokio::sync::mpsc::UnboundedSender<usize>,
    replies: tokio::sync::mpsc::Receiver<bytes::BytesMut>,
    requested_bytes: Option<usize>,
}

impl tokio::io::AsyncRead for SyncReadGateway {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let rem = buf.remaining();

        loop {
            if let Some(rq) = self.requested_bytes {
                if rq > rem {
                    panic!("SyncReadGateway's poll_read was called with suddenly a smaller buffer than before")
                }

                match self.replies.poll_recv(cx) {
                    std::task::Poll::Ready(None) => return std::task::Poll::Ready(std::io::Result::Ok(())),
                    std::task::Poll::Ready(Some(rb)) => {
                        assert!(rb.len() <= rq);
                        buf.put_slice(&*rb);
                        self.requested_bytes = None;
                        return std::task::Poll::Ready(std::io::Result::Ok(()));
                    }
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            } else {
                match self.reqests.send(rem) {
                    Ok(_) => {}
                    Err(_) => return std::task::Poll::Ready(std::io::Result::Err(std::io::ErrorKind::ConnectionAborted.into())),
                }
                self.requested_bytes = Some(rem);
            }
        }
    }
}
struct SyncWriteGateway {
    
}

impl tokio::io::AsyncWrite for SyncWriteGateway {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        todo!()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        todo!()
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<T: Node + Send + Sync + 'static> super::Node for T {
    async fn run(
        &self,
        ctx: RunContext,
        multiconn: Option<&mut IWantToServeAnotherConnection>,
    ) -> Result<super::Bipipe> {
        let (buffer_sizes_tx, mut buffer_sizes_rx) = tokio::sync::mpsc::unbounded_channel();
        let (buffers_tx, buffers_rx) = tokio::sync::mpsc::channel(1);

        let rg = SyncReadGateway {
            reqests: buffer_sizes_tx,
            replies: buffers_rx,
            requested_bytes: None,
        };
        let wg = SyncWriteGateway {}; 
        Node::run(self, ctx, multiconn.is_some(), move |pipe| {
            match pipe.r {
                Source::ByteStream(mut rr) => {
                    while let Some(b) = buffer_sizes_rx.blocking_recv() {
                        let mut bb = bytes::BytesMut::with_capacity(b);
                        bb.resize(b, 0);
                        match rr.read(&mut *bb) {
                            Ok(sz) => {
                                bb.truncate(sz);
                                if buffers_tx.blocking_send(bb).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Source::Datagrams(_) => {}
                Source::None => {}
            }

            Ok(())
        })?;
        Ok(super::Bipipe {
            r: super::Source::ByteStream(Box::pin(rg)),
            w: super::Sink::ByteStream(Box::pin(wg)),
            closing_notification: None,

        })
    }
}
