#[cfg(unix)]
extern crate tokio_file_unix;
extern crate tokio_reactor;
#[cfg(all(unix, feature = "signal_handler"))]
extern crate tokio_signal;
extern crate tokio_stdin_stdout;

use futures;
use futures::future::Future;
use std;
use std::cell::RefCell;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tokio_io::{AsyncRead, AsyncWrite};

#[cfg(unix)]
use self::tokio_file_unix::File as UnixFile;
use std::fs::{File as FsFile, OpenOptions};

use super::{BoxedNewPeerFuture, Peer, Result};
use futures::Stream;

use super::{once, spawn_hack, ConstructParams, PeerConstructor, Specifier};

#[derive(Clone, Debug)]
pub struct AsyncStdio;
impl Specifier for AsyncStdio {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        let ret;
        ret = get_stdio_peer(&mut p.global(GlobalState::default));
        once(ret)
    }
    specifier_boilerplate!(globalstate singleconnect no_subspec);
}

specifier_class!(
    name = AsyncStdioClass,
    target = AsyncStdio,
    prefixes = ["asyncstdio:"],
    arg_handling = noarg,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
[A] Set stdin and stdout to nonblocking mode, then use it as a communication counterpart. UNIX-only.
May cause problems with programs running at the same terminal. This specifier backs the `--async-stdio` CLI option. 

Typically this specifier can be specified only one time.
    
Example: simulate `cat(1)`. This is an exception from "only one time" rule above:

    websocat - -

Example: SSH transport

    ssh -c ProxyCommand='websocat asyncstdio: ws://myserver/mywebsocket' user@myserver
"#
);


specifier_class!(
    name = InetdClass,
    target = AsyncStdio,
    prefixes = ["inetd:"],
    arg_handling = noarg,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Like `asyncstdio:`, but intended for inetd(8) usage. [A]

Automatically enables `-q` (`--quiet`) mode.

`inetd-ws:` - is of `ws-l:inetd:`

Example of inetd.conf line that makes it listen for websocket
connections on port 1234 and redirect the data to local SSH server.

    1234 stream tcp nowait myuser  /opt/websocat websocat inetd-ws: tcp:127.0.0.1:22
"#
);

#[derive(Clone, Debug)]
pub struct OpenAsync(pub PathBuf);
impl Specifier for OpenAsync {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        let ret;
        ret = get_file_peer(&self.0);
        once(ret)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = OpenAsyncClass,
    target = OpenAsync,
    prefixes = ["open-async:"],
    arg_handling = into,
    overlay = false,
    MessageOriented, // ?
    SingleConnect,
    help = r#"
Open file for read and write and use it like a socket. [A]
Not for regular files, see readfile/writefile instead.
  
Example: Serve big blobs of random data to clients

    websocat -U ws-l:127.0.0.1:8088 open-async:/dev/urandom

"#
);

#[derive(Clone, Debug)]
pub struct OpenFdAsync(pub i32);
impl Specifier for OpenFdAsync {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        let ret;
        ret = get_fd_peer(self.0);
        once(ret)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = OpenFdAsyncClass,
    target = OpenFdAsync,
    prefixes = ["open-fd:"],
    arg_handling = parse,
    overlay = false,
    MessageOriented, // ?
    SingleConnect,
    help = r#"
Use specified file descriptor like a socket. [A]

Example: Serve random data to clients v2

    websocat -U ws-l:127.0.0.1:8088 reuse:open-fd:55   55< /dev/urandom
"#
);

fn get_stdio_peer_impl(s: &mut GlobalState) -> Result<Peer> {
    let si;
    let so;
    {
        if !UnixFile::raw_new(std::io::stdin()).get_nonblocking()? {
            debug!("Setting stdin to nonblocking mode");
            s.need_to_restore_stdin_blocking_status = true;
        }
        let stdin = self::UnixFile::new_nb(std::io::stdin())?;

        if !UnixFile::raw_new(std::io::stdout()).get_nonblocking()? {
            debug!("Setting stdout to nonblocking mode");
            s.need_to_restore_stdout_blocking_status = true;
        }
        let stdout = self::UnixFile::new_nb(std::io::stdout())?;

        si = stdin.into_reader(&tokio_reactor::Handle::default())?;
        so = stdout.into_io(&tokio_reactor::Handle::default())?;

        let s_clone = s.clone();

        #[cfg(all(unix, feature = "signal_handler"))]
        {
            debug!("Installing signal handler");
            let ctrl_c = tokio_signal::ctrl_c().flatten_stream();
            let prog = ctrl_c.for_each(move |()| {
                restore_blocking_status(&s_clone);
                ::std::process::exit(0);
                #[allow(unreachable_code)]
                Ok(())
            });
            spawn_hack(Box::new(prog.map_err(|_| ())));
        }
    }
    Ok(Peer::new(si, so, None))
}

pub fn get_stdio_peer(s: &mut GlobalState) -> BoxedNewPeerFuture {
    debug!("get_stdio_peer (async)");
    Box::new(futures::future::result(get_stdio_peer_impl(s))) as BoxedNewPeerFuture
}

#[derive(Default, Clone)]
pub struct GlobalState {
    need_to_restore_stdin_blocking_status: bool,
    need_to_restore_stdout_blocking_status: bool,
}

impl Drop for GlobalState {
    fn drop(&mut self) {
        restore_blocking_status(self);
    }
}

fn restore_blocking_status(s: &GlobalState) {
    {
        debug!("restore_blocking_status");
        if s.need_to_restore_stdin_blocking_status {
            debug!("Restoring blocking status for stdin");
            let _ = UnixFile::raw_new(std::io::stdin()).set_nonblocking(false);
        }
        if s.need_to_restore_stdout_blocking_status {
            debug!("Restoring blocking status for stdout");
            let _ = UnixFile::raw_new(std::io::stdout()).set_nonblocking(false);
        }
    }
}

type ImplPollEvented = ::tokio_reactor::PollEvented<UnixFile<std::fs::File>>;

#[derive(Clone)]
struct FileWrapper(Rc<RefCell<ImplPollEvented>>);

impl AsyncRead for FileWrapper {}
impl Read for FileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.0.borrow_mut().read(buf)
    }
}

impl AsyncWrite for FileWrapper {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.0.borrow_mut().shutdown()
    }
}
impl Write for FileWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.borrow_mut().flush()
    }
}

fn get_file_peer_impl(p: &Path) -> Result<Peer> {
    let oo = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(p)?;
    let f = self::UnixFile::new_nb(oo)?;

    let s = f.into_io(&tokio_reactor::Handle::default())?;
    let ss = FileWrapper(Rc::new(RefCell::new(s)));
    Ok(Peer::new(ss.clone(), ss, None))
}

pub fn get_file_peer(p: &Path) -> BoxedNewPeerFuture {
    debug!("get_file_peer");
    Box::new(futures::future::result(get_file_peer_impl(p))) as BoxedNewPeerFuture
}

fn get_fd_peer_impl(fd: i32) -> Result<Peer> {
    let ff: FsFile = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(fd) };
    let f = self::UnixFile::new_nb(ff)?;

    let s = f.into_io(&tokio_reactor::Handle::default())?;
    let ss = FileWrapper(Rc::new(RefCell::new(s)));
    Ok(Peer::new(ss.clone(), ss, None))
}

pub fn get_fd_peer(fd: i32) -> BoxedNewPeerFuture {
    debug!("get_fd_peer");
    Box::new(futures::future::result(get_fd_peer_impl(fd))) as BoxedNewPeerFuture
}
