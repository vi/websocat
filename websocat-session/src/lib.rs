pub mod copy;

pub async fn run(c: websocat_api::Session) -> websocat_api::Result<()> {
    let rc1 = websocat_api::RunContext {
        nodes: c.nodes.clone(),
        left_to_right_things_to_be_filled_in: None,
        left_to_right_things_to_read_from: None,
        globals: c.global_things.clone(),
    };

    let p1: websocat_api::Bipipe = c.nodes[c.left].run(rc1, None).await.unwrap();

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
    println!("bytes={}", bytes);
    Ok(())
}
