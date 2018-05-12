extern crate tokio_stdin_stdout;

use super::{BoxedNewPeerFuture, Peer};

use super::{once, Handle, Options, PeerConstructor, ProgramState, Specifier};

use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ThreadedStdio;
impl Specifier for ThreadedStdio {
    fn construct(&self, _: &Handle, _: &mut ProgramState, _opts: Rc<Options>) -> PeerConstructor {
        once(get_stdio_peer())
    }
    specifier_boilerplate!(globalstate singleconnect no_subspec typ=Stdio);
}


specifier_class!(
    name=ThreadedStdioClass, 
    target=ThreadedStdio, 
    prefixes=["threadedstdio:"], 
    arg_handling=noarg,
    help="TODO"
);
#[cfg(not(all(unix, not(feature = "no_unix_stdio"))))]
specifier_class!(
    name=ThreadedStdioSubstituteClass, 
    target=ThreadedStdio, 
    prefixes=["-","stdio:","inetd:"], 
    arg_handling=noarg,
    help="TODO"
);

pub fn get_stdio_peer() -> BoxedNewPeerFuture {
    info!("get_stdio_peer (threaded)");
    Box::new(::futures::future::ok(Peer::new(
        tokio_stdin_stdout::stdin(0),
        tokio_stdin_stdout::stdout(0),
    ))) as BoxedNewPeerFuture
}
