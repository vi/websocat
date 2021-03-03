pub mod copy;


fn rerun(c: websocat_api::Session, continuation: Option<websocat_api::AnyObject>) {
    tokio::spawn(async move {
        let _ = run_impl(c, continuation).await;
    });
}

//#[tracing::instrument(name="websocat_session::run", level="debug", skip(c), err)]
async fn run_impl(c: websocat_api::Session, continuation: Option<websocat_api::AnyObject>) -> websocat_api::Result<()> {
    if continuation.is_none() {
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
    let multiconn = websocat_api::ServerModeContext {
        you_are_called_not_the_first_time: continuation,
        call_me_again_with_this: Box::new(move |cont| {
            rerun(c2, Some(cont))
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
    Ok(())
}

pub fn run(c: websocat_api::Session) -> impl std::future::Future<Output=websocat_api::Result<()>> {
  run_impl(c, None)
}
