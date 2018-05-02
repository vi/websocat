extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

use tokio_core::reactor::{Core};
use websocat::{spec,serve,Reuser,StdioUsageStatus};

use StdioUsageStatus::{IsItself, WithReuser};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;


fn run() -> Result<()> {
    let arg1 = std::env::args().nth(1).ok_or(
        "Usage: websocat - ws[s]://...
Some examples:
    websocat - -
    websocat ws-l:tcp-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    websocat - ws-c:tcp:127.0.0.1:8080
Wacky mode:
    websocat ws-l:ws-l:ws-c:- tcp:127.0.0.1:5678
    (Excercise to the reader: manage to actually connect to it).
"
    )?;
    let arg2 = std::env::args().nth(2).ok_or("no second arg")?;
    
    let s1 = spec(&arg1)?;
    let mut s2 = spec(&arg2)?;
    
    if s1.stdio_usage_status() == IsItself && s2.stdio_usage_status() == IsItself {
        // Degenerate mode: just copy stdin to stdout and call it a day
        ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
        return Ok(())
    }
    
    if s1.stdio_usage_status() >= WithReuser && s2.stdio_usage_status() >= WithReuser {
        Err("Too many usages of stdin/stdout")?;
    }
    
    if s1.is_multiconnect() && s2.stdio_usage_status() > WithReuser {
        //Err("Stdin/stdout is used without a `reuse:` overlay.")?;
        eprintln!("Warning: replies on stdio get directed at random connected client");
        s2 = Box::new(Reuser(s2));
    }

    let mut core = Core::new()?;

    let opts = Default::default();
    let prog = serve(core.handle(), s1, s2, opts, std::rc::Rc::new(|e| {
        eprintln!("websocat: {}", e);
    }));
    core.run(prog).map_err(|()|"error running".to_string())?;
    Ok(())
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
