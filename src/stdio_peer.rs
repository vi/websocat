extern crate tokio_stdin_stdout;
#[cfg(unix)]
extern crate tokio_file_unix;
#[cfg(all(unix,feature="signal_handler"))]
extern crate tokio_signal;

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;
use std::path::{PathBuf,Path};
use std::rc::Rc;
use std::cell::RefCell;
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

#[cfg(unix)]
use self::tokio_file_unix::{File as UnixFile};
use ::std::fs::{File as FsFile, OpenOptions};

use super::{Peer, BoxedNewPeerFuture, Result};
use futures::Stream;


use super::{once,Specifier,ProgramState,PeerConstructor,Options};

#[derive(Clone,Debug)]
pub struct Stdio;
impl Specifier for Stdio {
    fn construct(&self, h:&Handle, ps: &mut ProgramState, _opts: &Options) -> PeerConstructor {
        let ret;
        ret = get_stdio_peer(&mut ps.stdio, h);
        once(ret)
    }
    specifier_boilerplate!(typ=Stdio globalstate singleconnect no_subspec);
}

#[derive(Clone,Debug)]
pub struct OpenAsync(pub PathBuf);
impl Specifier for OpenAsync {
    fn construct(&self, h:&Handle, _ps: &mut ProgramState, _opts: &Options) -> PeerConstructor {
        let ret;
        ret = get_file_peer(&self.0, h);
        once(ret)
    }
    specifier_boilerplate!(typ=Other noglobalstate singleconnect no_subspec);
}

#[derive(Clone,Debug)]
pub struct OpenFdAsync(pub i32);
impl Specifier for OpenFdAsync {
    fn construct(&self, h:&Handle, _ps: &mut ProgramState, _opts: &Options) -> PeerConstructor {
        let ret;
        ret = get_fd_peer(self.0, h);
        once(ret)
    }
    specifier_boilerplate!(typ=Other noglobalstate singleconnect no_subspec);
}




fn get_stdio_peer_impl(s: &mut GlobalState, handle: &Handle) -> Result<Peer> {
    let si;
    let so;
    {
        if !UnixFile::raw_new(std::io::stdin()).get_nonblocking()? {
            info!("Setting stdin to nonblocking mode");
            s.need_to_restore_stdin_blocking_status = true;
        }
        let stdin  = self::UnixFile::new_nb(std::io::stdin())?;
        
        if !UnixFile::raw_new(std::io::stdout()).get_nonblocking()? {
            info!("Setting stdout to nonblocking mode");
            s.need_to_restore_stdout_blocking_status = true;
        }
        let stdout = self::UnixFile::new_nb(std::io::stdout())?;
    
        si = stdin.into_reader(&handle)?;
        so = stdout.into_io(&handle)?;
        
        let s_clone = s.clone();
        
        #[cfg(all(unix,feature="signal_handler"))]
        {
            info!("Installing signal handler");
            let ctrl_c = tokio_signal::ctrl_c(&handle).flatten_stream();
            let prog = ctrl_c.for_each(move |()| {
                restore_blocking_status(&s_clone);
                ::std::process::exit(0);
                #[allow(unreachable_code)]
                Ok(())
            });
            handle.spawn(prog.map_err(|_|()));
        }
    }
    Ok(Peer::new(si,so))
}

pub fn get_stdio_peer(s: &mut GlobalState, handle: &Handle) -> BoxedNewPeerFuture {
    info!("get_stdio_peer (async)");
    Box::new(futures::future::result(get_stdio_peer_impl(s, handle))) as BoxedNewPeerFuture
}


#[derive(Default,Clone)]
pub struct GlobalState {
    need_to_restore_stdin_blocking_status : bool,
    need_to_restore_stdout_blocking_status: bool,
}

impl Drop for GlobalState {
    fn drop(&mut self) {
        restore_blocking_status(self);
    }
}

fn restore_blocking_status(s : &GlobalState) {
    {
        debug!("restore_blocking_status");
        if s.need_to_restore_stdin_blocking_status {
            info!("Restoring blocking status for stdin");
            let _ = UnixFile::raw_new(std::io::stdin()).set_nonblocking(false);
        }
        if s.need_to_restore_stdout_blocking_status {
            info!("Restoring blocking status for stdout");
            let _ = UnixFile::raw_new(std::io::stdout()).set_nonblocking(false);
        }
    }
}




type ImplPollEvented = ::tokio_core::reactor::PollEvented<UnixFile<std::fs::File>>;

#[derive(Clone)]
struct FileWrapper(Rc<RefCell<ImplPollEvented>>);

impl AsyncRead for FileWrapper{}
impl Read for FileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.0.borrow_mut().read(buf)
    }
}


impl AsyncWrite for FileWrapper {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
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

fn get_file_peer_impl(p: &Path, handle: &Handle) -> Result<Peer> {
    let oo = OpenOptions::new().read(true).write(true).create(false).open(p)?;
    let f = self::UnixFile::new_nb(oo)?;

    let s = f.into_io(&handle)?;
    let ss = FileWrapper(Rc::new(RefCell::new(s)));
    Ok(Peer::new(ss.clone(), ss))
}


pub fn get_file_peer(p: &Path, handle: &Handle) -> BoxedNewPeerFuture {
   info!("get_file_peer");
    Box::new(futures::future::result(get_file_peer_impl(p, handle))) as BoxedNewPeerFuture
}


fn get_fd_peer_impl(fd: i32, handle: &Handle) -> Result<Peer> {
    let ff : FsFile = unsafe { std::os::unix::io::FromRawFd::from_raw_fd(fd) };
    let f = self::UnixFile::new_nb(ff)?;

    let s = f.into_io(&handle)?;
    let ss = FileWrapper(Rc::new(RefCell::new(s)));
    Ok(Peer::new(ss.clone(), ss))
}


pub fn get_fd_peer(fd: i32, handle: &Handle) -> BoxedNewPeerFuture {
   info!("get_fd_peer");
    Box::new(futures::future::result(get_fd_peer_impl(fd, handle))) as BoxedNewPeerFuture
}
