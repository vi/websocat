extern crate tokio_stdin_stdout;
#[cfg(unix)]
extern crate tokio_file_unix;
#[cfg(unix)]
extern crate tokio_signal;

use std;
use tokio_core::reactor::{Handle};
use futures;
use futures::future::Future;

#[cfg(unix)]
use self::tokio_file_unix::{File as UnixFile};

use super::{Peer, BoxedNewPeerFuture, Result};
use futures::Stream;

fn get_stdio_peer_impl(handle: &Handle) -> Result<Peer> {
    let si;
    let so;
    
    #[cfg(any(not(unix),feature="no_unix_stdio"))]
    {
        si = tokio_stdin_stdout::stdin(0);
        so = tokio_stdin_stdout::stdout(0);
    }
    
    #[cfg(all(unix,not(feature="no_unix_stdio")))]
    {
        let stdin  = self::UnixFile::new_nb(std::io::stdin())?;
        let stdout = self::UnixFile::new_nb(std::io::stdout())?;
    
        si = stdin.into_reader(&handle)?;
        so = stdout.into_io(&handle)?;
        
        let ctrl_c = tokio_signal::ctrl_c(&handle).flatten_stream();
        let prog = ctrl_c.for_each(|()| {
            restore_blocking_status();
            ::std::process::exit(0);
            #[allow(unreachable_code)]
            Ok(())
        });
        handle.spawn(prog.map_err(|_|()));
    }
    Ok(Peer::new(si,so))
}

pub fn get_stdio_peer(handle: &Handle) -> BoxedNewPeerFuture {
    Box::new(futures::future::result(get_stdio_peer_impl(handle))) as BoxedNewPeerFuture
}

pub fn restore_blocking_status() {
    #[cfg(all(unix,not(feature="no_unix_stdio")))]
    {
        let _ = UnixFile::raw_new(std::io::stdin()).set_nonblocking(false);
        let _ = UnixFile::raw_new(std::io::stdout()).set_nonblocking(false);
    }
}
