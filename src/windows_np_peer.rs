extern crate tokio_named_pipes;

use futures;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};
use std::path::{Path, PathBuf};

//use super::{L2rUser, LeftSpecToRightSpec};

use std::cell::RefCell;
use std::rc::Rc;

use tokio_named_pipes::NamedPipe;


use super::{once, ConstructParams, PeerConstructor, Specifier};
use super::{BoxedNewPeerFuture, Peer, Result};

#[derive(Debug, Clone)]
pub struct NamedPipeConnect(pub PathBuf);
impl Specifier for NamedPipeConnect {
    fn construct(&self, _p: ConstructParams) -> PeerConstructor {
        once(Box::new(futures::future::result(named_pipe_connect_peer(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = NamedPipeConnectClass,
    target = NamedPipeConnect,
    prefixes = ["namedpipeconnect:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Connect to a named pipe on Windows

Example:

    websocat ws-l:127.0.0.1:8000 namedpipeconnect:\\.\pipe\Pipe

"#
);

fn named_pipe_connect_peer(
    path: &Path,
) -> Result<Peer> {
    let pipe = NamedPipe::new(path, &tokio::reactor::Handle::default())?;
    let ph = NamedPipeConnectPeer(Rc::new(RefCell::new(pipe)));
    Ok(Peer::new(ph.clone(), ph, None))   
}

#[derive(Clone)]
struct NamedPipeConnectPeer(Rc<RefCell<NamedPipe>>);

impl Read for NamedPipeConnectPeer {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0
            .borrow_mut()
            .read(buf)
    }
}

impl Write for NamedPipeConnectPeer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0
            .borrow_mut()
            .write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0
            .borrow_mut()
            .flush()
    }
}

impl AsyncRead for NamedPipeConnectPeer {}

impl AsyncWrite for NamedPipeConnectPeer {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self
            .0
            .borrow_mut()
            .shutdown()
    }
}
