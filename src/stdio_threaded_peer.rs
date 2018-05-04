extern crate tokio_stdin_stdout;

use super::{Peer, BoxedNewPeerFuture};

pub fn get_stdio_peer() -> BoxedNewPeerFuture {
    info!("get_stdio_peer (threaded)");
    Box::new(
        ::futures::future::ok(
            Peer::new(
                tokio_stdin_stdout::stdin(0),
                tokio_stdin_stdout::stdout(0),
            )
        )
    ) as BoxedNewPeerFuture
}
