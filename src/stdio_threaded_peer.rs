extern crate tokio_stdin_stdout;

use super::{Peer, BoxedNewPeerFuture};

use super::{once,Specifier,Handle,ProgramState,PeerConstructor};

#[derive(Debug)]
pub struct ThreadedStdio;
impl Specifier for ThreadedStdio {
    fn construct(&self, _:&Handle, _: &mut ProgramState) -> PeerConstructor {
        once(get_stdio_peer())
    }
    specifier_boilerplate!(singleconnect, Stdio);
}


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
