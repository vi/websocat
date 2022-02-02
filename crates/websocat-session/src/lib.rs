#![allow(clippy::option_if_let_else)]
pub mod copy;

use std::sync::{atomic::AtomicUsize, Arc};

use tracing::Instrument;
use websocat_api::{NodeId, Tree, anyhow::{self, Context}, async_trait::async_trait, futures};
use websocat_derive::WebsocatNode;

/// this ball is passed around from session to session
struct Ball {
    /// session number
    i: usize,

    /// Receiver is at session with i=0.
    /// Sender is passed around from one sesion to another.
    /// If it is ever dropped, the first task (i=0) would know
    /// that is safe to exit, as no multiconn-ignited sessions can be created
    vigilance_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// AtomicUsize counts currently running parallel sessions (through the read lock)
    /// Write lock is only used to wait when all other sessions quit, so we are safe
    /// to exit from the first session
    ctr: std::sync::Arc<tokio::sync::RwLock<std::sync::atomic::AtomicUsize>>,
}

fn rerun(
    opts: Opts,
    c: Session,
    continuation: Option<websocat_api::AnyObject>,
    ball: Ball,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = run_impl(opts, c, continuation, ball).await;
    })
}

#[tracing::instrument(name="half", level="debug", skip(r,w,dir),  fields(d=tracing::field::display(dir)), err)]
async fn half_session(
    dir: &'static str,
    r: websocat_api::Source,
    w: websocat_api::Sink,
) -> websocat_api::Result<()> {
    match (r, w) {
        (websocat_api::Source::ByteStream(mut r), websocat_api::Sink::ByteStream(mut w)) => {
            tracing::debug!("A bytestream session");
            let bytes = copy::copy(&mut r, &mut w).await?;
            tracing::info!(
                "Finished Websocat byte transfer session. Processed {} bytes",
                bytes
            );
        }
        (websocat_api::Source::Datagrams(r), websocat_api::Sink::Datagrams(w)) => {
            use futures::stream::StreamExt;
            tracing::debug!("A datagram session");
            let counter = Arc::new(AtomicUsize::new(0));
            let counter_ = counter.clone();
            let r = r.inspect(|_| {
                counter_.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            });
            match r.forward(w).await {
                Ok(()) => {
                    tracing::info!(
                        "Finished Websocat datagram transfer session. Processed {} datagrams",
                        counter.load(std::sync::atomic::Ordering::SeqCst)
                    );
                }
                Err(e) => {
                    tracing::info!(
                        "Finished Websocat datagram transfer session with error. Source emitted {} datagrams",
                        counter.load(std::sync::atomic::Ordering::SeqCst)
                    );
                    return Err(e);
                }
            }
        }
        (websocat_api::Source::None, websocat_api::Sink::None) => {
            tracing::info!("Finished Websocat dummy transfer session.",);
        }
        (websocat_api::Source::Datagrams(_), websocat_api::Sink::ByteStream(_)) => {
            anyhow::bail!("Failed to connect datagram-based node to a bytestream-based node")
        }
        (websocat_api::Source::ByteStream(_), websocat_api::Sink::Datagrams(_)) => {
            anyhow::bail!("Failed to connect bytestream-based node to a datagram-based node")
        }
        (websocat_api::Source::None, _) => {
            anyhow::bail!(
                "Failed to interconnect an unreadable node to a node that expects some writing"
            )
        }
        (_, websocat_api::Sink::None) => {
            anyhow::bail!(
                "Failed to interconnect an unwriteable node to a node that expects some reading"
            )
        }
    };
    Ok(())
}

#[tracing::instrument(name="session", level="debug", skip(c,continuation,ball,opts),  fields(i=tracing::field::display(ball.i)), err)]
async fn run_impl(
    opts: Opts,
    c: Session,
    continuation: Option<websocat_api::AnyObject>,
    ball: Ball,
) -> websocat_api::Result<()> {
    let first_sesion = continuation.is_none();
    if first_sesion {
        tracing::info!("Running a Websocat session");
    } else {
        tracing::info!("Running additional Websocat session");
    }

    let rc1 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
    };

    let c2 = c.clone();
    let enable_multiple_connections = opts.enable_multiple_connections;
    let (vigilance_tx, vigilance_rx) = if !enable_multiple_connections {
        (None, None)
    } else if let Some(tx) = ball.vigilance_tx {
        (Some(tx), None)
    } else {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        (Some(tx), Some(rx))
    };

    let enable_forward = opts.enable_forward;
    let enable_backward = opts.enable_backward;

    let i = ball.i;
    let ctr2 = ball.ctr.clone();
    let multiconn = if let Some(vigilance_tx) = vigilance_tx {
        Some(websocat_api::ServerModeContext {
            you_are_called_not_the_first_time: continuation,
            call_me_again_with_this: Box::new(move |cont| {
                rerun(
                    opts,
                    c2,
                    Some(cont),
                    Ball {
                        i: i + 1,
                        vigilance_tx: Some(vigilance_tx),
                        ctr: ctr2,
                    },
                );
            }),
        })
    } else {
        None
    };

    let readlock = ball.ctr.read().await;
    let parallel = readlock.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    tracing::debug!("Now running {} parallel sessions", parallel);

    let try_block = async move {
        let n1 = c.nodes[c.left]
            .clone()
            .upgrade()
            .with_context(|| format!("Trying to run the left node"))?;
        let p1: websocat_api::Bipipe = websocat_api::RunnableNode::run(n1, rc1, multiconn).await?;

        let rc2 = websocat_api::RunContext {
            nodes: c.nodes.clone(),
            left_to_right_things_to_be_filled_in: None,
            left_to_right_things_to_read_from: None,
        };

        let n2 = c.nodes[c.right]
            .clone()
            .upgrade()
            .with_context(|| format!("Trying to run the right node"))?;
        let p2: websocat_api::Bipipe = websocat_api::RunnableNode::run(n2, rc2, None).await?;

        match (enable_forward, enable_backward) {
            (true, true) => {
                let span = tracing::Span::current();
                let t = tokio::spawn(half_session("<", p2.r, p1.w).instrument(span));
                half_session(">", p1.r, p2.w).await?;
                t.await??;
            }
            (true, false) => {
                half_session(">", p1.r, p2.w).await?;
                drop(p2.r);
                drop(p1.w);
            }
            (false, true) => {
                half_session("<", p2.r, p1.w).await?;
                drop(p2.w);
                drop(p1.r);
            }
            (false, false) => {
                tracing::info!("Finished a dummy session with both forward and backward transfer directions disabled");
            }
        }

        Ok::<(), anyhow::Error>(())
    };

    if let Err(e) = try_block.await {
        tracing::error!("Session finished with error: {:#}", e);
    }

    let parallel2 = readlock.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1;
    tracing::debug!("Now running {} parallel sessions", parallel2);

    drop(readlock);

    if let Some(vigilance_rx) = vigilance_rx {
        tracing::debug!(
            "This looks like the first sesion. Waiting for the listener to finish listening."
        );
        let _ = vigilance_rx.await;
        tracing::debug!(
            "No more pending connections to be listened. Now checking for running parallel sessions."
        );

        let writelock = ball.ctr.write().await;
        if writelock.load(std::sync::atomic::Ordering::SeqCst) != 0 {
            tracing::error!(
                "Somehow obtained a write lock while there are also parallel sessions running?"
            );
            // hang and wait endlessly - better than mistakingly interrupting sessions
            futures::future::pending::<()>().await;
        }

        tracing::debug!("No more parallel sessions. Should be safe to exit now.");
    } else {
        if enable_multiple_connections {
            tracing::debug!("Safe to just exit this session, as it is not the first one");
        } else {
            tracing::debug!("Safe to just exit this session, as we in --oneshot mode");
        }
    }
    Ok(())
}

pub struct Opts {
    pub enable_forward: bool,
    pub enable_backward: bool,
    pub enable_multiple_connections: bool,
}

pub fn run(
    opts: Opts,
    c: Session,
) -> impl std::future::Future<Output = websocat_api::Result<()>> {
    run_impl(
        opts,
        c,
        None,
        Ball {
            i: 0,
            vigilance_tx: None,
            ctr: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::sync::atomic::AtomicUsize::new(0),
            )),
        },
    )
}

#[derive(Debug, Clone, WebsocatNode)]
#[websocat_node(official_name = "session")]
#[auto_populate_in_allclasslist]
pub struct SessionClass {
    /// Left or listerer part of the session specifier
    pub left: NodeId,

    /// Right or connector part of the session specifier
    pub right: NodeId,

    /// Do not pass bytes or datagrams from left to right
    #[cli="unidirectional"]
    #[websocat_prop(default=false)]
    pub unidirectional: bool,

    /// Do not pass bytes or datagrams from right to left
    #[cli="unidirectional-reverse"]
    #[websocat_prop(default=false)]
    pub unidirectional_reverse: bool,

    /// Inhibit rerunning of left node
    #[cli="oneshot"]
    #[websocat_prop(default=false)]
    pub oneshot: bool,
}

#[derive(Clone)]
pub struct Session {
    pub nodes: Arc<Tree>,
    pub left: NodeId,
    pub right: NodeId,
}

#[async_trait]
impl websocat_api::RunnableNode for SessionClass {
    async fn run(
        self: std::pin::Pin<Arc<Self>>,
        ctx: websocat_api::RunContext,
        _: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {

        let opts = Opts {
            enable_forward: !self.unidirectional,
            enable_backward: !self.unidirectional_reverse,
            enable_multiple_connections: !self.oneshot,
        };
        let sess = Session {
            nodes: ctx.nodes.clone(),
            left: self.left,
            right: self.right,
        };

        run(opts, sess).await?;

        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::None,
            w: websocat_api::Sink::None,
            closing_notification: None,
        })
    }
}
