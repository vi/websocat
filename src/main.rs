extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

use tokio_core::reactor::{Core};
use futures::future::Future;
use websocat::{Session,peer_from_str,ProgramState,is_stdio_peer,is_stdioish_peer};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;


fn run() -> Result<()> {
    let mut ps : ProgramState = Default::default();

    let arg1 = std::env::args().nth(1).ok_or("Usage: websocat - ws[s]://...")?;
    let arg2 = std::env::args().nth(2).ok_or("no second arg")?;

    if is_stdio_peer(arg1.as_ref()) && is_stdio_peer(arg2.as_ref()) {
        // Degenerate mode: just copy stdin to stdout and call it a day
        ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
        return Ok(())
    }
    
    if is_stdioish_peer(arg1.as_ref()) && is_stdioish_peer(arg2.as_ref()) {
        Err("Too many usages of stdin/stdout")?;
    }

    let mut core = Core::new()?;
    let handle = core.handle();

    let h1 = core.handle();
    let h2 = core.handle();

    let runner = peer_from_str(&mut ps, &h1, arg1.as_ref())
    .and_then(|ws_peer| {
        peer_from_str(&mut ps, &h2, arg2.as_ref())
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

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
