extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

use tokio_core::reactor::{Core};
use futures::future::Future;
use websocat::{Session,peer_from_str};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;


fn run() -> Result<()> {
    let arg1 = std::env::args().nth(1).ok_or("Usage: websocat - ws[s]://...")?;
    let arg2 = std::env::args().nth(2).ok_or("no second arg")?;

    let mut core = Core::new()?;
    let handle = core.handle();

    let h1 = core.handle();
    let h2 = core.handle();

    let runner = peer_from_str(&h1, arg1.as_ref())
    .and_then(|ws_peer| {
        peer_from_str(&h2, arg2.as_ref())
        .and_then(|std_peer| {
            let s = Session::new(ws_peer,std_peer);
            
            s.run(&handle)
                .map(|_|())
                .map_err(|_|unreachable!())
        })
    });

    core.run(runner)?;
    Ok(())
}

fn main() {
    let r = run();

    websocat::stdio_peer::restore_blocking_status();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
