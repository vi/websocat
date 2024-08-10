use std::{ffi::OsString, io::ErrorKind, pin::Pin, task::Poll};

use crate::scenario_executor::utils::TaskHandleExt2;
use rhai::{Engine, FnPtr, NativeCallContext};
use tokio::{
    io::AsyncWrite,
    process::{Child, ChildStdin, Command},
};
use tracing::{debug, warn};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

use super::{types::Hangup, utils::{ExtractHandleOrFail, HandleExt, RhResult, SimpleErr}};

//@ Prepare subprocess, setting up executable name.
fn subprocess_new(program_name: String) -> Handle<Command> {
    Some(Command::new(program_name)).wrap()
}

//@ Prepare subprocess, setting up possibly non-UTF8 executable name
fn subprocess_new_osstr(program_name: OsString) -> Handle<Command> {
    Some(Command::new(program_name)).wrap()
}

//@ Add one command line argument to the array
fn subprocess_arg(ctx: NativeCallContext, cmd: &mut Handle<Command>, arg: String) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    c.arg(arg);
    cmd.put(c);
    Ok(())
}

//@ Add one possibly non-UTF8 command line argument to the array
fn subprocess_arg_osstr(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    arg: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    c.arg(arg);
    cmd.put(c);
    Ok(())
}

//@ Configure what to do with subprocess's stdin, stdout and stderr
//@
//@ Each numeric argument accepts the following values:
//@ * `0` meaning the fd will be /dev/null-ed.
//@ * `1` meaning leave it connected to Websocat's fds.
//@ * `2` meaning we can capture process's input or output.
fn subprocess_configure_fds(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    stdin: i64,
    stdout: i64,
    stderr: i64,
) -> RhResult<()> {
    use std::process::Stdio;
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    let gets = |x: i64| -> RhResult<Stdio> {
        Ok(match x {
            0 => Stdio::null(),
            1 => Stdio::inherit(),
            2 => Stdio::piped(),
            _ => return Err(ctx.err("Invalid value for subprocess_configure_fds argument")),
        })
    };
    let (si, so, se) = (gets(stdin)?, gets(stdout)?, gets(stderr)?);

    c.stdin(si).stdout(so).stderr(se);

    cmd.put(c);
    Ok(())
}

//@ Execute the prepared subprocess and wait for its exit code
//@ Callback receives exit code or `-1` meaning that starting failed
//@ or `-2` meaning the process exited because of signal
fn subprocess_execute_for_status(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let mut c = ctx.lutbarm(cmd)?;
    let the_scenario = ctx.get_scenario()?;
    Ok(async move {
        debug!("starting subprocess");

        let s = c.status().await;

        let ret = match s {
            Ok(x) => match x.code() {
                Some(x) => x.into(),
                None => -2,
            },
            Err(e) => {
                warn!("Failed to execute subprocess: {e}");
                -1
            }
        };

        callback_and_continue::<(i64,)>(the_scenario, continuation, (ret,)).await;
        Ok(())
    }
    .wrap())
}

//@ Execute the prepared subprocess and wait for its exit, storing
//@ output of stdout and stderr in memory.
//@ Status code the callback receives follows similar rules as in `subprocess_execute_for_status`.
//@ Second and third arguments of the callback are stdout and stderr respectively.
fn subprocess_execute_for_output(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let mut c = ctx.lutbarm(cmd)?;
    let the_scenario = ctx.get_scenario()?;
    Ok(async move {
        debug!("starting subprocess");

        let o = c.output().await;

        let (code, stdout, stderr) = match o {
            Ok(x) => {
                let code = match x.status.code() {
                    Some(x) => x.into(),
                    None => -2,
                };
                (code, x.stdout, x.stderr)
            }
            Err(e) => {
                warn!("Failed to execute subprocess: {e}");
                (-1, vec![], vec![])
            }
        };

        callback_and_continue::<(i64, Vec<u8>, Vec<u8>)>(
            the_scenario,
            continuation,
            (code, stdout, stderr),
        )
        .await;
        Ok(())
    }
    .wrap())
}

struct StdinWrapper(Option<ChildStdin>);

impl AsyncWrite for StdinWrapper {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if let Some(ref mut x) = self.get_mut().0 {
            Pin::new(x).poll_write(cx, buf)
        } else {
            Poll::Ready(Err(ErrorKind::BrokenPipe.into()))
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if let Some(ref mut x) = self.get_mut().0 {
            Pin::new(x).poll_flush(cx)
        } else {
            Poll::Ready(Err(ErrorKind::BrokenPipe.into()))
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if let Some(x) = self.get_mut().0.take() {
            drop(x);
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(ErrorKind::BrokenPipe.into()))
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        if let Some(ref mut x) = self.get_mut().0 {
            Pin::new(x).poll_write_vectored(cx, bufs)
        } else {
            Poll::Ready(Err(ErrorKind::BrokenPipe.into()))
        }
    }

    fn is_write_vectored(&self) -> bool {
        if let Some(ref x) = self.0 {
            x.is_write_vectored()
        } else {
            true
        }
    }
}

//@ Convert the child process handle to a Stream Socket of its stdin and stdout (but not stderr).
//@ In case of non-piped (`2`) fds, the resulting socket would be incomplete.
fn child_socket(
    ctx: NativeCallContext,
    chld: &mut Handle<Child>,
) -> RhResult<Handle<StreamSocket>> {
    let (mut c, chld) = ctx.lutbar2m(chld)?;
    let s = StreamSocket {
        read: c.stdout.take().map(|x| StreamRead {
            reader: Box::pin(x),
            prefix: bytes::BytesMut::new(),
        }),
        write: c.stdin.take().map(|x| StreamWrite {
            writer: Box::pin(StdinWrapper(Some(x))),
        }),
        close: None,
    };

    debug!(s=?s, "subprocess socket");

    chld.put(c);
    Ok(Some(s).wrap())
}

//@ Obtain a Hangup handle that resovles when child process terminates.
//@ `Child` instance cannot be used after this.
fn child_wait(
    ctx: NativeCallContext,
    chld: &mut Handle<Child>,
) -> RhResult<Handle<Hangup>> {
    let mut c = ctx.lutbarm(chld)?;
    let s : Hangup  = Box::pin(async move {
        match c.wait().await {
            Ok(x) => {
                debug!("child process exited with status {x}")
            }
            Err(e) => {
                warn!("Failed to wait for a child process: {e}")
            }
        }
    });

    Ok(Some(s).wrap())
}

//@ Terminate a child process.
//@ `Child` instance cannot be used after this.
fn child_kill(
    ctx: NativeCallContext,
    chld: &mut Handle<Child>,
) -> RhResult<Handle<Hangup>> {
    let mut c = ctx.lutbarm(chld)?;
    let s : Hangup  = Box::pin(async move {
        match c.kill().await {
            Ok(()) => {
                debug!("child process terminated")
            }
            Err(e) => {
                warn!("Failed to terminate a child process: {e}")
            }
        }
    });

    Ok(Some(s).wrap())
}

//@ Take stderr handle as a Stream Reader (i.e. half-socket).
//@ In case of non-piped (`2`) fds, the handle would be null
fn child_take_stderr(
    ctx: NativeCallContext,
    chld: &mut Handle<Child>,
) -> RhResult<Handle<StreamRead>> {
    let (mut c, chld) = ctx.lutbar2m(chld)?;

    let s = c.stderr.take().map(|x| StreamRead {
        reader: Box::pin(x),
        prefix: bytes::BytesMut::new(),
    });

    chld.put(c);
    Ok(s.wrap())
}


//@ Spawn the prepared subprocess. What happens next depends on used `child_` function.
fn subprocess_spawn(ctx: NativeCallContext, cmd: &mut Handle<Command>) -> RhResult<Handle<Child>> {
    let mut c = ctx.lutbarm(cmd)?;
    match c.spawn() {
        Ok(x) => {
            debug!("spawned subprocess");
            Ok(Some(x).wrap())
        }
        Err(e) => {
            warn!("Process spawning failed: {e}");
            Err(ctx.err("Failed to spawn the process"))
        }
    }
}


//@ Add literal, unescaped text to Window's command line
#[allow(unused)]
fn subprocess_raw_windows_arg(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    arg: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    
    #[cfg(not(windows))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(windows)] {
        c.raw_arg(arg);
    }

    cmd.put(c);
    Ok(())
}

//@ Add or set environtment variable for the subprocess
fn subprocess_env(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    key: String,
    value: String,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.env(key, value);

    cmd.put(c);
    Ok(())
}

//@ Add or set environtment variable for the subprocess (possibly non-UTF8)
fn subprocess_env_osstr(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    key: OsString,
    value: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.env(key, value);

    cmd.put(c);
    Ok(())
}

//@ Add or set environtment variable for the subprocess.
fn subprocess_env_remove(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    key: String,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.env_remove(key);

    cmd.put(c);
    Ok(())
}

//@ Add or set environtment variable for the subprocess.
fn subprocess_env_remove_osstr(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    key: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.env_remove(key);

    cmd.put(c);
    Ok(())
}

//@ Clear all environment variables for the subprocess.
fn subprocess_env_clear(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.env_clear();

    cmd.put(c);
    Ok(())
}

//@ Change current directory for the subprocess.
fn subprocess_chdir(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    dir: String,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.current_dir(dir);

    cmd.put(c);
    Ok(())
}

//@ Change current directory for the subprocess.
fn subprocess_chdir_osstr(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    dir: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    c.current_dir(dir);

    cmd.put(c);
    Ok(())
}

//@ Set Window's process creation flags.
#[allow(unused)]
fn subprocess_windows_creation_flags(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    flags: i64,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    let flags : u32 = flags as u32;
    #[cfg(not(windows))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(windows)] {
        c.creation_flags(flags);
    }

    cmd.put(c);
    Ok(())
}

//@ Set subprocess's uid on Unix.
#[allow(unused)]
fn subprocess_uid(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    uid: i64,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    let x : u32 = uid as u32;
    #[cfg(not(unix))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(unix)] {
        c.uid(x);
    }

    cmd.put(c);
    Ok(())
}

//@ Set subprocess's uid on Unix.
#[allow(unused)]
fn subprocess_gid(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    gid: i64,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;

    let x : u32 = gid as u32;
    #[cfg(not(unix))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(unix)] {
        c.gid(x);
    }

    cmd.put(c);
    Ok(())
}

//@ Override process's name / zeroeth command line argument on Unix.
#[allow(unused)]
fn subprocess_arg0(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    arg0: String,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    #[cfg(not(unix))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(unix)] {
        c.arg0(arg0);
    }

    cmd.put(c);
    Ok(())
}

//@ Override process's name / zeroeth command line argument on Unix.
#[allow(unused)]
fn subprocess_arg0_osstr(
    ctx: NativeCallContext,
    cmd: &mut Handle<Command>,
    arg0: OsString,
) -> RhResult<()> {
    let (mut c, cmd) = ctx.lutbar2m(cmd)?;
    #[cfg(not(unix))] {
        return Err(ctx.err("This function is not available on this platform"));
    }

    #[cfg(unix)] {
        c.arg0(arg0);
    }

    cmd.put(c);
    Ok(())
}


pub fn register(engine: &mut Engine) {
    engine.register_fn("subprocess_new", subprocess_new);
    engine.register_fn("subprocess_new_osstr", subprocess_new_osstr);
    engine.register_fn("arg", subprocess_arg);
    engine.register_fn("arg_osstr", subprocess_arg_osstr);
    engine.register_fn("configure_fds", subprocess_configure_fds);
    engine.register_fn("execute_for_status", subprocess_execute_for_status);
    engine.register_fn("execute_for_output", subprocess_execute_for_output);
    engine.register_fn("execute", subprocess_spawn);
    engine.register_fn("socket", child_socket);
    engine.register_fn("take_stderr", child_take_stderr);
    engine.register_fn("wait", child_wait);
    engine.register_fn("kill", child_kill);
    engine.register_fn("raw_windows_arg", subprocess_raw_windows_arg);

    engine.register_fn("env", subprocess_env);
    engine.register_fn("env_osstr", subprocess_env_osstr);
    engine.register_fn("env_remove", subprocess_env_remove);
    engine.register_fn("env_remove_osstr", subprocess_env_remove_osstr);
    engine.register_fn("env_clear", subprocess_env_clear);
    engine.register_fn("chdir", subprocess_chdir);
    engine.register_fn("chdir_osstr", subprocess_chdir_osstr);
    engine.register_fn("windows_creation_flags", subprocess_windows_creation_flags);
    engine.register_fn("uid", subprocess_uid);
    engine.register_fn("gid", subprocess_gid);
    engine.register_fn("arg0", subprocess_arg0);
    engine.register_fn("arg0_osstr", subprocess_arg0_osstr);
}
