extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

use tokio_core::reactor::{Core};
use futures::future::Future;
use futures::Stream;
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

    let h1 = core.handle();
    let h2 = core.handle();
    
    use websocat::PeerConstructor::{ServeMultipleTimes, ServeOnce};

    let left = peer_from_str(&mut ps, &h1, arg1.as_ref());
    match left {
        ServeMultipleTimes(stream) => {
            let runner = stream
            .map(|peer1| {
                h2.spawn(
                    peer_from_str(&mut ps, &h2, arg2.as_ref())
                    .get_only_first_conn()
                    .and_then(move |peer2| {
                        let s = Session::new(peer1,peer2);
                        s.run()
                    })
                    .map_err(|e| {
                        eprintln!("websocat: {}", e);
                    })
                )
            }).for_each(|()|futures::future::ok(()));
            core.run(runner)?;
        },
        ServeOnce(peer1c) => {
            let runner = peer1c
            .and_then(|peer1| {
                let right = peer_from_str(&mut ps, &h2, arg2.as_ref());
                let fut = right.get_only_first_conn();
                fut.and_then(move |peer2| {
                    let s = Session::new(peer1,peer2);
                    s.run()
                })
            });
            core.run(runner)?;
        },
    };

    Ok(())
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
