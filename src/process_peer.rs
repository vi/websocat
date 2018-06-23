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

use super::{once, ConstructParams, PeerConstructor, Specifier};
use super::{BoxedNewPeerFuture, Peer};
use std::process::Stdio;

#[derive(Debug, Clone)]
pub struct ShC(pub String);
impl Specifier for ShC {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let args = if cfg!(target_os = "windows") {
            let mut args = Command::new("cmd");
            args.arg("/C").arg(self.0.clone());
            args
        } else {
            let mut args = Command::new("sh");
            args.arg("-c").arg(self.0.clone());
            args
        };
        let h = &p.tokio_handle;
        once(Box::new(futures::future::result(process_connect_peer(h, args))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}
specifier_class!(
    name = ShCClass,
    target = ShC,
    prefixes = ["sh-c:", "cmd:"], // TODO: change semantics of sh-c:
    arg_handling = into,
    help = r#"
Start specified command line using `sh -c` or `cmd /C`
  
Example: serve a counter

    websocat -U ws-l:127.0.0.1:8008 cmd:'for i in 0 1 2 3 4 5 6 7 8 9 10; do echo $i; sleep 1; done'
  
Example: unauthenticated shell

    websocat --exit-on-eof ws-l:127.0.0.1:5667 sh-c:'bash -i 2>&1'

"#
);
// TODO: client and example output for each server example
// TODO: chromium-based examples

#[derive(Debug, Clone)]
pub struct Exec(pub String);
impl Specifier for Exec {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let mut args = Command::new(self.0.clone());
        args.args(p.program_options.exec_args.clone());
        let h = &p.tokio_handle;
        once(Box::new(futures::future::result(process_connect_peer(h, args))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec typ=Other);
}
specifier_class!(
    name = ExecClass,
    target = Exec,
    prefixes = ["exec:"],
    arg_handling = into,
    help = r#"
Execute a program directly (without a subshell), providing array of arguments on Unix

Example: Serve current date

  websocat -U ws-l:127.0.0.1:5667 exec:date
  
Example: pinger

  websocat -U ws-l:127.0.0.1:5667 exec:ping --exec-args 127.0.0.1 -c 1
  
"#
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
