extern crate tokio_process;

use futures;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::{self, process::ExitStatus};
use tokio_io::{AsyncRead, AsyncWrite};

use super::{L2rUser, LeftSpecToRightSpec};

use std::cell::RefCell;
use std::rc::Rc;

use std::process::Command;

use self::tokio_process::{Child, CommandExt};

use super::{once, ConstructParams, PeerConstructor, Specifier};
use super::{BoxedNewPeerFuture, Peer};
use std::process::Stdio;

fn needenv(p: &ConstructParams) -> Option<&LeftSpecToRightSpec> {
    match (p.program_options.exec_set_env, &p.left_to_right) {
        (true, &L2rUser::ReadFrom(ref x)) => Some(&**x),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Cmd(pub String);
impl Specifier for Cmd {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let zero_sighup = p.program_options.process_zero_sighup;
        let exit_sighup = p.program_options.process_exit_sighup;
        let exit_on_disconnect = p.program_options.process_exit_on_disconnect;
        let args = if cfg!(target_os = "windows") {
            let mut args = Command::new("cmd");
            args.arg("/C").arg(self.0.clone());
            args
        } else {
            let mut args = Command::new("sh");
            args.arg("-c").arg(self.0.clone());
            args
        };
        let env = needenv(&p);
        once(Box::new(futures::future::result(process_connect_peer(
            args,
            env,
            zero_sighup,
            exit_sighup,
            exit_on_disconnect,
        ))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = CmdClass,
    target = Cmd,
    prefixes = ["cmd:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Start specified command line using `sh -c` or `cmd /C` (depending on platform)

Otherwise should be the the same as `sh-c:` (see examples from there).
"#
);
// TODO: client and example output for each server example
// TODO: chromium-based examples

#[derive(Debug, Clone)]
pub struct ShC(pub String);
impl Specifier for ShC {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let zero_sighup = p.program_options.process_zero_sighup;
        let exit_sighup = p.program_options.process_exit_sighup;
        let exit_on_disconnect = p.program_options.process_exit_on_disconnect;
        let mut args = Command::new("sh");
        args.arg("-c").arg(self.0.clone());
        let env = needenv(&p);
        once(Box::new(futures::future::result(process_connect_peer(
            args,
            env,
            zero_sighup,
            exit_sighup,
            exit_on_disconnect,
        ))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = ShCClass,
    target = ShC,
    prefixes = ["sh-c:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Start specified command line using `sh -c` (even on Windows)
  
Example: serve a counter

    websocat -U ws-l:127.0.0.1:8008 sh-c:'for i in 0 1 2 3 4 5 6 7 8 9 10; do echo $i; sleep 1; done'
  
Example: unauthenticated shell

    websocat --exit-on-eof ws-l:127.0.0.1:5667 sh-c:'bash -i 2>&1'
"#
);

#[derive(Debug, Clone)]
pub struct Exec(pub String);
impl Specifier for Exec {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let zero_sighup = p.program_options.process_zero_sighup;
        let exit_sighup = p.program_options.process_exit_sighup;
        let exit_on_disconnect = p.program_options.process_exit_on_disconnect;
        let mut args = Command::new(self.0.clone());
        args.args(p.program_options.exec_args.clone());
        let env = needenv(&p);
        once(Box::new(futures::future::result(process_connect_peer(
            args,
            env,
            zero_sighup,
            exit_sighup,
            exit_on_disconnect,
        ))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = ExecClass,
    target = Exec,
    prefixes = ["exec:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Execute a program directly (without a subshell), providing array of arguments on Unix [A]

Example: Serve current date

  websocat -U ws-l:127.0.0.1:5667 exec:date
  
Example: pinger

  websocat -U ws-l:127.0.0.1:5667 exec:ping --exec-args 127.0.0.1 -c 1
  
"#
);

fn process_connect_peer(
    mut cmd: Command,
    l2r: Option<&LeftSpecToRightSpec>,
    zero_sighup: bool,
    close_sighup: bool,
    exit_on_disconnect: bool,
) -> Result<Peer, Box<dyn std::error::Error>> {
    if let Some(x) = l2r {
        if let Some(ref z) = x.client_addr {
            cmd.env("WEBSOCAT_CLIENT", z);
        };
        if let Some(ref z) = x.uri {
            cmd.env("WEBSOCAT_URI", z);
        };
        for (hn, hv) in &x.headers {
            cmd.env(format!("H_{}", hn), hv);
        }
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
    let child = cmd.spawn_async()?;
    let ph = ProcessPeer {
        chld: Rc::new(RefCell::new(ForgetfulProcess {
            chld: Some(child),
            exit_on_disconnect,
        })),
        sighup_on_zero: zero_sighup,
        sighup_on_close: close_sighup,
    };
    Ok(Peer::new(ph.clone(), ph, None /* TODO */))
}

struct ForgetfulProcess {
    chld: Option<Child>,
    exit_on_disconnect: bool,
}
#[derive(Clone)]
struct ProcessPeer {
    chld: Rc<RefCell<ForgetfulProcess>>,
    sighup_on_zero: bool,
    sighup_on_close: bool,
}

impl Read for ProcessPeer {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.chld
            .borrow_mut()
            .chld
            .as_mut()
            .unwrap()
            .stdout()
            .as_mut()
            .expect("assertion failed 1425")
            .read(buf)
    }
}

impl Write for ProcessPeer {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        #[cfg(unix)]
        {
            if self.sighup_on_zero && buf.is_empty() {
                // TODO use nix crate?
                if let Some(ref chld) = self.chld.borrow().chld {
                    unsafe {
                        extern crate libc;
                        libc::kill(chld.id() as libc::pid_t, libc::SIGHUP);
                    }
                }
            }
        }
        self.chld
            .borrow_mut()
            .chld
            .as_mut()
            .unwrap()
            .stdin()
            .as_mut()
            .expect("assertion failed 1425")
            .write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.chld
            .borrow_mut()
            .chld
            .as_mut()
            .unwrap()
            .stdin()
            .as_mut()
            .expect("assertion failed 1425")
            .flush()
    }
}

impl AsyncRead for ProcessPeer {}

impl AsyncWrite for ProcessPeer {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        #[cfg(unix)]
        {
            if self.sighup_on_close {
                // TODO use nix crate?
                if let Some(ref chld) = self.chld.borrow().chld {
                    unsafe {
                        extern crate libc;
                        libc::kill(chld.id() as libc::pid_t, libc::SIGHUP);
                    }
                }
            }
        }
        debug!("Shutdown of process peer's writer");
        let mut c: tokio_process::ChildStdin = self
            .chld
            .borrow_mut()
            .chld
            .as_mut()
            .unwrap()
            .stdin()
            .take()
            .expect("assertion failed 1425");
        c.shutdown()
    }
}

impl Drop for ForgetfulProcess {
    fn drop(&mut self) {
        use futures::Future;
        let mut chld = self.chld.take().unwrap();
        if self.exit_on_disconnect {
            debug!("Forcing child process to exit");
            match chld.kill() {
                Ok(()) => (),
                Err(e) => {
                    warn!("Error terminating child process: {}", e);
                }
            }
            tokio::spawn(
                chld.map(|_exc: ExitStatus| {

                }).map_err(|_|())
            );
        } else {
            tokio::spawn(
                chld.map(|exc: ExitStatus| {
                    if exc.success() {
                        debug!("Child process exited")
                    } else {
                        warn!("Child process exited unsuccessfully: {:?}", exc.code());
                    }
                })
                .map_err(|e| {
                    error!("Error waiting for child process termination: {}", e);
                }),
            );
        }
    }
}
