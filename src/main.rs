extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

use tokio_core::reactor::{Core};
use websocat::{spec,serve};

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
    let s2 = spec(&arg2)?;
    
    if s1.is_stdio() && s2.is_stdio() {
        // Degenerate mode: just copy stdin to stdout and call it a day
        ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
        return Ok(())
    }
    
    if s1.directly_uses_stdio() && s2.directly_uses_stdio() {
        Err("Too many usages of stdin/stdout")?;
    }
    
    if s1.is_multiconnect() && s2.directly_uses_stdio() {
        Err("Stdin/stdout is used without a `reuse:` overlay.")?;
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
