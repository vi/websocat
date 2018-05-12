extern crate tokio_process;

use futures;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

use std::cell::RefCell;
use std::rc::Rc;

use std::process::Command;

use self::tokio_process::{Child, CommandExt};

use super::{BoxedNewPeerFuture, Peer};
use super::{once, Options, PeerConstructor, ProgramState, Specifier};
use std::process::Stdio;

#[derive(Debug, Clone)]
pub struct ShC(pub String);
impl Specifier for ShC {
    fn construct(&self, h: &Handle, _: &mut ProgramState, _opts: Rc<Options>) -> PeerConstructor {
        let args = if cfg!(target_os = "windows") {
            let mut args = Command::new("cmd");
            args.arg("/C").arg(self.0.clone());
            args
        } else {
            let mut args = Command::new("sh");
            args.arg("-c").arg(self.0.clone());
            args
        };
        once(Box::new(futures::future::result(process_connect_peer(h, args))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}
specifier_class!(
    name=ShCClass, 
    target=ShC, 
    prefixes=["sh-c:", "cmd:"], 
    arg_handling=into,
    help="TODO"
);

#[derive(Debug, Clone)]
pub struct Exec(pub String);
impl Specifier for Exec {
    fn construct(&self, h: &Handle, _: &mut ProgramState, opts: Rc<Options>) -> PeerConstructor {
        let mut args = Command::new(self.0.clone());
        args.args(opts.exec_args.clone());
        once(Box::new(futures::future::result(process_connect_peer(h, args))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}
specifier_class!(
    name=ExecClass, 
    target=Exec, 
    prefixes=["exec:"], 
    arg_handling=into,
    help="TODO"
);

fn process_connect_peer(h: &Handle, mut cmd: Command) -> Result<Peer, Box<std::error::Error>> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
    let child = cmd.spawn_async(h)?;
    let ph = ProcessPeer(Rc::new(RefCell::new(child)));
    Ok(Peer::new(ph.clone(), ph))
}

#[derive(Clone)]
struct ProcessPeer(Rc<RefCell<Child>>);

impl Read for ProcessPeer {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.0
            .borrow_mut()
            .stdout()
            .as_mut()
            .expect("assertion failed 1425")
            .read(buf)
    }
}

impl Write for ProcessPeer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0
            .borrow_mut()
            .stdin()
            .as_mut()
            .expect("assertion failed 1425")
            .write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.0
            .borrow_mut()
            .stdin()
            .as_mut()
            .expect("assertion failed 1425")
            .flush()
    }
}

impl AsyncRead for ProcessPeer {}

impl AsyncWrite for ProcessPeer {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.0
            .borrow_mut()
            .stdin()
            .as_mut()
            .expect("assertion failed 1425")
            .shutdown()
    }
}
