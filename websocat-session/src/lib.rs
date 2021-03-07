#![allow(clippy::option_if_let_else)]
pub mod copy;

use websocat_api::{anyhow, futures};

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
    c: websocat_api::Session,
    continuation: Option<websocat_api::AnyObject>,
    ball: Ball,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = run_impl(c, continuation, ball).await;
    })
}


#[tracing::instrument(name="half", level="debug", skip(r,w),  fields(d=tracing::field::display(dir)), err)]
async fn half_session(dir:&'static str, r: websocat_api::Source, w : websocat_api::Sink) -> websocat_api::Result<()> {
    match (r, w) {
        (websocat_api::Source::ByteStream(mut r), websocat_api::Sink::ByteStream(mut w)) => {
            tracing::debug!("A bytestream session");
            let bytes = copy::copy(&mut r, &mut w).await.unwrap();
            tracing::info!(
                "Finished Websocat byte transfer session. Processed {} bytes",
                bytes
            );
        }
        (websocat_api::Source::Datagrams(r), websocat_api::Sink::Datagrams(w)) => {
            use futures::stream::StreamExt;
            tracing::debug!("A datagram session");
            r.forward(w).await?;
            tracing::info!(
                "Finished Websocat datagram transfer session. Processed {} datagrams",
                '?'
            );
        }
        (websocat_api::Source::None, websocat_api::Sink::None) => {
            tracing::info!(
                "Finished Websocat dummy transfer session.",
            );
        }
        (websocat_api::Source::Datagrams(_), websocat_api::Sink::ByteStream(_)) => {
            anyhow::bail!("Failed to connect datagram-based node to a bytestream-based node")
        }
        (websocat_api::Source::ByteStream(_), websocat_api::Sink::Datagrams(_)) => {
            anyhow::bail!("Failed to connect bytestream-based node to a datagram-based node")
        }
        (websocat_api::Source::None, _) => {
            anyhow::bail!("Failed to interconnect an unreadable node to a node that expects some writing")
        }
        (_, websocat_api::Sink::None) => {
            anyhow::bail!("Failed to interconnect an unwriteable node to a node that expects some reading")
        }
    };
    Ok(())
}

#[tracing::instrument(name="session", level="debug", skip(c,continuation,ball),  fields(i=tracing::field::display(ball.i)), err)]
async fn run_impl(
    c: websocat_api::Session,
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
        globals: c.global_things.clone(),
    };

    let c2 = c.clone();
    let (vigilance_tx, vigilance_rx) = if let Some(tx) = ball.vigilance_tx {
        (tx, None)
    } else {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        (tx, Some(rx))
    };

    let i = ball.i;
    let ctr2 = ball.ctr.clone();
    let multiconn = websocat_api::ServerModeContext {
        you_are_called_not_the_first_time: continuation,
        call_me_again_with_this: Box::new(move |cont| {
            rerun(
                c2,
                Some(cont),
                Ball {
                    i: i + 1,
                    vigilance_tx: Some(vigilance_tx),
                    ctr: ctr2,
                },
            );
        }),
    };

    let readlock = ball.ctr.read().await;
    let parallel = readlock.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    tracing::debug!("Now running {} parallel sessions", parallel);

    let p1: websocat_api::Bipipe = c.nodes[c.left].run(rc1, Some(multiconn)).await.unwrap();

    let rc2 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
        globals: c.global_things.clone(),
    };

    let p2: websocat_api::Bipipe = c.nodes[c.right].run(rc2, None).await.unwrap();

    let t = tokio::spawn(half_session("<", p2.r, p1.w));
    half_session(">", p1.r, p2.w).await?;
    t.await??;


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
                "Somehow obtained write lock while there are also parallel sessions running?"
            );
            futures::future::pending::<()>().await;
        }

        tracing::debug!("No more parallel sessions. Should be safe to exit now.");
    } else {
        tracing::debug!("Safe to just exit this session, as it is not the first one");
    }
    Ok(())
}

pub fn run(
    c: websocat_api::Session,
) -> impl std::future::Future<Output = websocat_api::Result<()>> {
    run_impl(
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
