pub mod copy;

/// this ball is passed around from session to session
struct Ball {
    /// session number
    i : usize,

    /// Receiver is at session with i=0.
    /// Sender is passed around from one sesion to another.
    /// If it is ever dropped, the first task (i=0) would know
    /// that is safe to exit
    vigilance_tx: Option<tokio::sync::oneshot::Sender<()>>,
}


fn rerun(c: websocat_api::Session, continuation: Option<websocat_api::AnyObject>, ball : Ball) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = run_impl(c, continuation, ball).await;
    })
}

#[tracing::instrument(name="websocat_session::run", level="debug", skip(c,continuation,ball),  fields(i=tracing::field::display(ball.i)), err)]
async fn run_impl(c: websocat_api::Session, continuation: Option<websocat_api::AnyObject>, ball:Ball) -> websocat_api::Result<()> {
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
    let (vigilance_tx, vigilance_rx) = match ball.vigilance_tx {
        Some(tx) => {
            (tx, None)
        }
        None => {
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            (tx, Some(rx)) 
        }
    };
    
    let i = ball.i;
    let multiconn = websocat_api::ServerModeContext {
        you_are_called_not_the_first_time: continuation,
        call_me_again_with_this: Box::new(move |cont| {
            rerun(c2, Some(cont), Ball {
                i: i+1,
                vigilance_tx: Some(vigilance_tx),
            });
        }),
    };

    let p1: websocat_api::Bipipe = c.nodes[c.left].run(rc1, Some(multiconn)).await.unwrap();

    let rc2 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
        globals: c.global_things.clone(),
    };

    let p2 : websocat_api::Bipipe = c.nodes[c.right].run(rc2, None).await.unwrap();

    let (mut r,mut w) = match (p1.r, p2.w) {
        (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => (r,w),
        _ => panic!(),
    };

    let bytes = copy::copy(&mut r, &mut w).await.unwrap();
    tracing::info!("Finished Websocat session. Processed {} bytes", bytes);
    if let Some(vigilance_rx) = vigilance_rx {
        tracing::debug!("This looks like the first sesion. Waiting for possible ongoing connections to finish.");
        let _ = vigilance_rx.await;
        tracing::debug!("No more pending connections to be listened. Assuming it is safe to exit now");
    } else {
        tracing::debug!("Safe to just exit this session, as it is not the first one");
    }
    Ok(())
}

pub fn run(c: websocat_api::Session) -> impl std::future::Future<Output=websocat_api::Result<()>> {
  run_impl(c, None, Ball { i: 0, vigilance_tx: None })
}
