use super::{DataNode, Result, RunContext};

pub enum Source {
    ByteStream(Box<dyn std::io::Read + Send + 'static>),
    Datagrams(Box<dyn FnMut() -> Result<Option<bytes::Bytes>> + Send + 'static>),
    None,
}

pub enum Sink {
    ByteStream(Box<dyn std::io::Write + Send + 'static>),
    Datagrams(Box<dyn FnMut(bytes::Bytes) -> Result<()> + Send + 'static>),
    None,
}

pub struct Bipipe {
    pub r: Source,
    pub w: Sink,
    pub closing_notification: Option<tokio::sync::oneshot::Receiver<()>>,
}
pub trait Node: DataNode {
    /// Started from a Tokio runtime thread, so don't block it, spawn your own thread to handle things.
    /// If this is a server that does multiple connections, start `closure` in a loop.
    /// The `closure` is supposed to run in a thread that can block
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: RunContext,
        allow_multiconnect: bool,
        closure: impl FnMut(Bipipe) -> Result<()> + Send + 'static,
    ) -> Result<()>;
}


/// Utility struct to be able to share `Read` and `Write` in two threads
/// (for things whose shared references implement `Read` and/or `Write`)
#[derive(Debug)]
pub struct ArcReadWrite<T>(std::sync::Arc<T>);

impl<T> Clone for ArcReadWrite<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> ArcReadWrite<T> {
    pub fn new(x: T) -> Self {
        ArcReadWrite(std::sync::Arc::new(x))
    }
}

impl<T> std::io::Read for ArcReadWrite<T>
where
    for<'a> &'a T: std::io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.as_ref().read(buf)
    }
}

impl<T> std::io::Write for ArcReadWrite<T>
where
    for<'a> &'a T: std::io::Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.as_ref().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.as_ref().flush()
    }
}    

#[cfg(not(feature="sync_impl"))]
#[async_trait::async_trait]
impl<T: Node + Send + Sync + 'static> crate::Node for T {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: RunContext,
        _multiconn: Option<crate::ServerModeContext>,
    ) -> Result<crate::Bipipe> {
        anyhow::bail!("Cargo feature websocat-api/sync_impl is not enabled")
    }
}

#[cfg(feature="sync_impl")]
mod syncimpl {
    use std::pin::Pin;
    use super::{RunContext, Node, Source, Result, Sink};
    use crate::ServerModeContext;
    use crate::{Bipipe as AsyncBipipe};
    struct SyncReadGateway {
        reqests: tokio::sync::mpsc::UnboundedSender<usize>,
        replies: tokio::sync::mpsc::Receiver<std::io::Result<bytes::Bytes>>,
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
                        std::task::Poll::Ready(None) => {
                            return std::task::Poll::Ready(std::io::Result::Ok(()))
                        }
                        std::task::Poll::Ready(Some(Ok(rb))) => {
                            assert!(rb.len() <= rq);
                            buf.put_slice(&*rb);
                            self.requested_bytes = None;
                            return std::task::Poll::Ready(std::io::Result::Ok(()));
                        }
                        std::task::Poll::Ready(Some(Err(e))) => {
                            self.requested_bytes = None;
                            return std::task::Poll::Ready(std::io::Result::Err(e));
                        }
                        std::task::Poll::Pending => return std::task::Poll::Pending,
                    }
                } else {
                    match self.reqests.send(rem) {
                        Ok(_) => {}
                        Err(_) => {
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ))
                        }
                    }
                    self.requested_bytes = Some(rem);
                }
            }
        }
    }
    
    impl SyncReadGateway {
        #[tracing::instrument(name = "SRG", skip(rr))]
        fn run(mut rr: impl std::io::Read + Send + 'static) -> SyncReadGateway {
            let (buffer_sizes_tx, mut buffer_sizes_rx) = tokio::sync::mpsc::unbounded_channel();
            let (buffers_tx, buffers_rx) = tokio::sync::mpsc::channel(1);
    
            let rg = SyncReadGateway {
                reqests: buffer_sizes_tx,
                replies: buffers_rx,
                requested_bytes: None,
            };
    
            std::thread::spawn(move || {
                let span = tracing::trace_span!("SRG_thread");
    
                'outer: while let Some(b) = buffer_sizes_rx.blocking_recv() {
                    tracing::trace!(parent: &span, "Received read request for buffer size {}", b);
                    let mut bb = bytes::BytesMut::with_capacity(b);
                    bb.resize(b, 0);
    
                    loop {
                        match rr.read(&mut *bb) {
                            Ok(sz) => {
                                tracing::debug!(
                                    parent: &span,
                                    "Underlying std::io::Read::read returned {} bytes",
                                    sz
                                );
                                bb.truncate(sz);
                                if buffers_tx.blocking_send(Ok(bb.freeze())).is_err() {
                                    tracing::debug!("Failed to sent to SyncReadGateway");
                                    break 'outer;
                                }
                                tracing::trace!(parent: &span, "Finished sending the reply buffer");
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                                tracing::debug!(
                                    parent: &span,
                                    "Received 'Interrupted'. Immediately retrying."
                                );
                                continue;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                tracing::warn!(parent: &span, "Received unexpected 'WouldBlock' from supposedly sync node. Waiting a bit and retrying.");
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                continue;
                            }
                            Err(e) => {
                                tracing::trace!(
                                    parent: &span,
                                    "Underlying std::io::Read::read failed: {}",
                                    e
                                );
                                if buffers_tx.blocking_send(Err(e)).is_err() {
                                    tracing::debug!("Also failed to sent to SyncReadGateway");
                                }
                                break 'outer;
                            }
                        }
                        break;
                    }
                }
                tracing::debug!(parent: &span, "Finished the thread");
            });
    
            rg
        }
    }
    
    struct SyncWriteGateway {
        reqests: tokio::sync::mpsc::UnboundedSender<SWGRequest>,
        replies: tokio::sync::mpsc::Receiver<Result<usize, std::io::Error>>,
        request_submitted: Option<SWGRequestTag>,
    }
    
    enum SWGRequest {
        Write(Box<[u8]>),
        Flush,
        Shutdown,
    }
    
    #[derive(Debug)]
    enum SWGRequestTag {
        Write,
        Flush,
        Shutdown,
    }
    
    impl tokio::io::AsyncWrite for SyncWriteGateway {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            loop {
                match &self.request_submitted {
                    None => match self.reqests.send(SWGRequest::Write(buf.into())) {
                        Ok(()) => {
                            self.request_submitted = Some(SWGRequestTag::Write);
                        }
                        Err(_) => {
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ))
                        }
                    },
                    Some(SWGRequestTag::Write) => match self.replies.poll_recv(cx) {
                        std::task::Poll::Ready(None) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ));
                        }
                        std::task::Poll::Ready(Some(Ok(sz))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Ok(sz));
                        }
                        std::task::Poll::Ready(Some(Err(e))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(e));
                        }
                        std::task::Poll::Pending => return std::task::Poll::Pending,
                    },
                    Some(x) => {
                        panic!("SyncWriteGateway was suddenly called for Write when previous {:?} has not reached to it's conclusion", x);
                    }
                }
            }
        }
    
        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            loop {
                match &self.request_submitted {
                    None => match self.reqests.send(SWGRequest::Flush) {
                        Ok(()) => {
                            self.request_submitted = Some(SWGRequestTag::Flush);
                        }
                        Err(_) => {
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ))
                        }
                    },
                    Some(SWGRequestTag::Flush) => match self.replies.poll_recv(cx) {
                        std::task::Poll::Ready(None) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ));
                        }
                        std::task::Poll::Ready(Some(Ok(_))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Ok(()));
                        }
                        std::task::Poll::Ready(Some(Err(e))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(e));
                        }
                        std::task::Poll::Pending => return std::task::Poll::Pending,
                    },
                    Some(x) => {
                        panic!("SyncWriteGateway was suddenly called for Flush when previous {:?} has not reached to it's conclusion", x);
                    }
                }
            }
        }
    
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            loop {
                match &self.request_submitted {
                    None => match self.reqests.send(SWGRequest::Shutdown) {
                        Ok(()) => {
                            self.request_submitted = Some(SWGRequestTag::Shutdown);
                        }
                        Err(_) => {
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ))
                        }
                    },
                    Some(SWGRequestTag::Shutdown) => match self.replies.poll_recv(cx) {
                        std::task::Poll::Ready(None) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(
                                std::io::ErrorKind::ConnectionAborted.into(),
                            ));
                        }
                        std::task::Poll::Ready(Some(Ok(_))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Ok(()));
                        }
                        std::task::Poll::Ready(Some(Err(e))) => {
                            self.request_submitted = None;
                            return std::task::Poll::Ready(std::io::Result::Err(e));
                        }
                        std::task::Poll::Pending => return std::task::Poll::Pending,
                    },
                    Some(x) => {
                        panic!("SyncWriteGateway was suddenly called for Shutdown when previous {:?} has not reached to it's conclusion", x);
                    }
                }
            }
        }
    }
    
    impl SyncWriteGateway {
        #[tracing::instrument(name = "SWG", skip(rr))]
        fn run(mut rr: impl std::io::Write + Send + 'static) -> SyncWriteGateway {
            let (requests_tx, mut requests_rx) = tokio::sync::mpsc::unbounded_channel();
            let (replies_tx, replies_rx) = tokio::sync::mpsc::channel(1);
    
            let wg = SyncWriteGateway {
                reqests: requests_tx,
                replies: replies_rx,
                request_submitted: None,
            };
    
            std::thread::spawn(move || {
                let span = tracing::trace_span!("SWG_thread");
    
                'outer: while let Some(rq) = requests_rx.blocking_recv() {
                    loop {
                        macro_rules! handle_errs {
                            ($ex:expr, $name:expr, $bind:ident, $succ:block) => {
                                match $ex {
                                    Ok($bind) => $succ,
                                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                                        tracing::debug!(parent: &span, "Received 'Interrupted'. Immediately retrying.");
                                        continue;
                                    }
                                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                        tracing::warn!(parent: &span, "Received unexpected 'WouldBlock' from supposedly sync node. Waiting a bit and retrying.");
                                        std::thread::sleep(std::time::Duration::from_millis(200));
                                        continue;
                                    }
                                    Err(e) => {
                                        tracing::trace!(parent: &span, "Underlying {} failed: {}", $name, e);
                                        if replies_tx.blocking_send(Err(e)).is_err() {
                                            tracing::debug!("Also failed to sent to SyncWriteGateway");
                                        }
                                        break 'outer
                                    }
                                }
                            }
                        }
                        match rq {
                            SWGRequest::Write(ref b) => {
                                tracing::trace!(
                                    parent: &span,
                                    "Received write request for size {}",
                                    b.len()
                                );
                                handle_errs!(rr.write(&*b), "std::io::Write::write", sz, {
                                    tracing::debug!(
                                        parent: &span,
                                        "Underlying std::io::Write::write returned {} bytes",
                                        sz
                                    );
                                    if replies_tx.blocking_send(Ok(sz)).is_err() {
                                        tracing::debug!("Failed to sent to SyncWriteGateway");
                                        break 'outer;
                                    }
                                    tracing::trace!(parent: &span, "Finished sending the reply");
                                });
                            }
                            SWGRequest::Flush => {
                                tracing::trace!(parent: &span, "Received flush request");
                                handle_errs!(rr.flush(), "std::io::Write::flush", _z, {
                                    tracing::debug!(
                                        parent: &span,
                                        "Underlying std::io::Write::flush returned"
                                    );
                                    if replies_tx.blocking_send(Ok(0)).is_err() {
                                        tracing::debug!("Failed to sent to SyncWriteGateway");
                                        break 'outer;
                                    }
                                    tracing::trace!(parent: &span, "Finished sending the reply");
                                });
                            }
                            SWGRequest::Shutdown => {
                                tracing::debug!(
                                    parent: &span,
                                    "Received shutdown request. Exiting thread."
                                );
                                if replies_tx.blocking_send(Ok(0)).is_err() {
                                    tracing::debug!("Failed to sent to SyncWriteGateway");
                                }
                                break 'outer;
                            }
                        }
                        break;
                    }
                }
                tracing::debug!(parent: &span, "Finished the thread");
            });
    
            wg
        }
    }
    
    struct SyncStreamGateway;
    
    impl SyncStreamGateway {
        fn run(
            rr: Box<dyn FnMut() -> Result<Option<bytes::Bytes>> + Send + 'static>,
        ) -> impl futures::stream::Stream<Item = Result<bytes::Bytes>> {
            //use futures::stream::StreamExt;
            //let r = std::sync::Arc::new(std::sync::Mutex::new(rr));
            futures::stream::unfold(rr, move |mut r| {
                async move {
                    let (r, ret) = match tokio::task::spawn_blocking(move || {
                        let ret = r();
                        (r,ret)
                    } ).await {
                        Ok(x) => x,
                        Err(e) => {
                            tracing::error!("Joing error: {}", e);
                            return None;
                        }
                    };
                    match ret {
                        Ok(Some(x)) => Some((Ok(x), r)),
                        Ok(None) => {
                            tracing::debug!("End of sync datagram stream");
                            None
                        }
                        Err(e) => {
                            tracing::error!("{}", e);
                            Some((Err(e), r))
                        }
                    }
                }
            })
            /*
            futures::stream::repeat(()).then(move |()| {
                let r = r.clone();
                async move {
                    //let r = r.clone();
                    match tokio::task::spawn_blocking(move || (r.lock().unwrap())()).await {
                        Ok(x) => x,
                        Err(e) => Err(e.into()),
                    }
                }
            })
            */
        }
    }
    
    struct SyncSinkGateway;
    
    impl SyncSinkGateway {
        fn run(
            ww: Box<dyn FnMut(bytes::Bytes) -> Result<()> + Send + 'static>,
        ) -> impl futures::sink::Sink<bytes::Bytes, Error=anyhow::Error> {
            use futures::sink::SinkExt;
            let w = std::sync::Arc::new(std::sync::Mutex::new(ww));
            futures::sink::drain().with(move |buf: bytes::Bytes| {
                let w = w.clone();
                async move {
                    //let w = w.clone();
                    match tokio::task::spawn_blocking(move || (w.lock().unwrap())(buf)).await {
                        Ok(x) => x,
                        Err(e) => Err(e.into()),
                    }
                }
            })
        }
    }
    
    #[async_trait::async_trait]
    impl<T: Node + Send + Sync + 'static> crate::RunnableNode for T {
        #[tracing::instrument(name = "SyncNode", level = "debug", skip(ctx, self, multiconn), err)]
        async fn run(
            self: std::pin::Pin<std::sync::Arc<Self>>,
            ctx: RunContext,
            mut multiconn: Option<ServerModeContext>,
        ) -> Result<AsyncBipipe> {
            let mut rx: Option<tokio::sync::mpsc::Receiver<AsyncBipipe>> = None;
    
            let allow_multiconnect = multiconn.is_some();
    
            if let Some(ref mut mc) = multiconn {
                if let Some(ref mut cag) = mc.you_are_called_not_the_first_time {
                    let tmp = cag
                        .downcast_mut::<Option<tokio::sync::mpsc::Receiver<AsyncBipipe>>>()
                        .expect("Unexpected object passed to restarted SyncNode::run");
                    rx = Some(tmp.take().unwrap());
                }
            }
            
            let mut nonfirst_connection = false;
    
            if rx.is_none() {
                if allow_multiconnect {
                    tracing::debug!(
                        "Initializing SyncNode for the first time in multiple connection series"
                    );
                } else {
                    tracing::debug!("Initializing SyncNode for serving one connection");
                }
    
                let (tx, rx_) = tokio::sync::mpsc::channel(1);
                rx = Some(rx_);
    
                Node::run(self, ctx, allow_multiconnect, move |pipe| {
                    let r = match pipe.r {
                        Source::ByteStream(rr) => {
                            let rg = SyncReadGateway::run(rr);
                            crate::Source::ByteStream(Box::pin(rg))
                        }
                        Source::Datagrams(rr) => {
                            let strgw = SyncStreamGateway::run(rr);
                            crate::Source::Datagrams(Box::pin(strgw))
                        }
                        Source::None => crate::Source::None,
                    };
    
                    let w = match pipe.w {
                        Sink::ByteStream(ww) => {
                            let wg = SyncWriteGateway::run(ww);
                            crate::Sink::ByteStream(Box::pin(wg))
                        }
                        Sink::Datagrams(ww) => {
                            let sinkgw = SyncSinkGateway::run(ww);
                            crate::Sink::Datagrams(Box::pin(sinkgw))
                        }
                        Sink::None => crate::Sink::None,
                    };
    
                    let bipipe = AsyncBipipe {
                        r,
                        w,
                        closing_notification: None,
                    };
                    if tx.blocking_send(bipipe).is_err() {
                        anyhow::bail!("Failed to send the bipipe to async world");
                    }
    
                    Ok(())
                })?;
            } else {
                tracing::debug!("Restored SyncNode's received from multiconnect context");
                nonfirst_connection = true;
            }
    
            let mut rx = rx.unwrap();
    
            let bipipe = rx
                .recv()
                .await;
            let bipipe = bipipe
                .ok_or_else(|| 
                    if ! nonfirst_connection {
                        anyhow::anyhow!("Failed to receive a bipipe from sync")
                    } else {
                        anyhow::anyhow!("No more connections from this sync node. Use --oneshot option to inhibit this error.")
                    }
            )?;
    
            tracing::debug!("Received bipipe");
    
            if let Some(mc) = multiconn {
                (mc.call_me_again_with_this)(Box::new(Some(rx)));
            }
    
            Ok(bipipe)
        }
    }
}
