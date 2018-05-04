extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

extern crate env_logger;


use tokio_core::reactor::{Core};
use websocat::{spec, WebsocatConfiguration};

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
    
    let opts = Default::default();
    
    let s1 = spec(&arg1)?;
    let s2 = spec(&arg2)?;
    
    let mut websocat = WebsocatConfiguration { opts, s1, s2 };
    
    if let Some(concern) = websocat.get_concern() {
        use websocat::ConfigurationConcern::*;
        if concern == StdinToStdout {
            // Degenerate mode: just copy stdin to stdout and call it a day
            ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
            return Ok(())
        }
        
        if concern == StdioConflict {
            Err("Too many usages of stdin/stdout")?;
        }
        
        if concern == NeedsStdioReuser {
            //Err("Stdin/stdout is used without a `reuse:` overlay.")?;
            eprintln!("Warning: replies on stdio get directed at random connected client");
            websocat = websocat.auto_install_reuser();
        }
    }

    let mut core = Core::new()?;

    let prog = websocat.serve(core.handle(), std::rc::Rc::new(|e| {
        eprintln!("websocat: {}", e);
    }));
    core.run(prog).map_err(|()|"error running".to_string())?;
    Ok(())
}

fn main() {
    env_logger::init();
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
