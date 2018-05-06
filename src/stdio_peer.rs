extern crate tokio_stdin_stdout;
#[cfg(unix)]
extern crate tokio_file_unix;
#[cfg(all(unix,feature="signal_handler"))]
extern crate tokio_signal;

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;

#[cfg(unix)]
use self::tokio_file_unix::{File as UnixFile};

use super::{Peer, BoxedNewPeerFuture, Result};
use futures::Stream;


use super::{once,Specifier,ProgramState,PeerConstructor};

#[derive(Clone,Debug)]
pub struct Stdio;
impl Specifier for Stdio {
    fn construct(&self, h:&Handle, ps: &mut ProgramState) -> PeerConstructor {
        let ret;
        ret = get_stdio_peer(&mut ps.stdio, h);
        once(ret)
    }
    specifier_boilerplate!(singleconnect, no_subspec, Stdio);
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
