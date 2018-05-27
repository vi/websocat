extern crate websocat;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_timer;

use futures::future::Future;

use tokio_core::reactor::Core;
use websocat::{spec, WebsocatConfiguration, Options};

fn dflt() -> Options {
    Default::default()
}

macro_rules! wt {
    ($core:ident, $s1:expr, $s2:expr, noopts, delay=$ms:expr) => {
        wt!($core, $s1, $s2, opts=Default::default(), delay=$ms);
    };
    ($core:ident, $s1:expr, $s2:expr, noopts, nodelay) => {
        wt!($core, $s1, $s2, opts=Default::default(), nodelay);
    };
    ($core:ident, $s1:expr, $s2:expr, opts=$opts:expr, delay=$ms:expr) => {{
        let s1 = spec($s1).unwrap();
        let s2 = spec($s2).unwrap();
        let h2 = $core.handle();

        let websocat = WebsocatConfiguration {
            opts: $opts,
            s1,
            s2,
        };

        let t = tokio_timer::wheel().build();
        let delay = t.sleep(std::time::Duration::new(0, $ms*1_000_000))
            .map_err(|_| ());

        delay.and_then(|()| {
            wt!(wss, websocat, h2)
        })
    }};
    ($core:ident, $s1:expr, $s2:expr, opts=$opts:expr, nodelay) => {{
        let s1 = spec($s1).unwrap();
        let s2 = spec($s2).unwrap();
        let h2 = $core.handle();

        let websocat = WebsocatConfiguration {
            opts: $opts,
            s1,
            s2,
        };
      
        wt!(wss, websocat, h2)
    }};
    (wss, $websocat:ident, $h2:ident) => {
        $websocat.serve(
                $h2,
                std::rc::Rc::new(|e| {
                    eprintln!("{}", e);
                    panic!();
                }),
            )
    };
}

macro_rules! prepare {
    ($core:ident) => {
        let _ = env_logger::try_init();
        let mut $core = Core::new().unwrap();
    };
}
macro_rules! run {
    ($core:ident, $prog:expr) => {
        $core.run($prog).map_err(|()| panic!()).unwrap();
    };
}

#[test]
fn trivial() {
    prepare!(core);
    let prog = wt!(core,
        "literal:qwerty",
        "assert:qwerty",
        noopts,
        nodelay);
    run!(core, prog);
}

#[test]
fn tcp() {
    prepare!(core);
    let prog1 = wt!(core,
        "literal:qwert2y",
        "tcp-l:127.0.0.1:45912",
        noopts,
        nodelay);
    let prog2 = wt!(core,
        "tcp:127.0.0.1:45912",
        "assert:qwert2y",
        noopts,
        delay=200);

    let prog = prog1.join(prog2);
    run!(core, prog);
}

#[test]
fn ws() {
    prepare!(core);
    let prog1 = wt!(core,
        "literal:qwert3y",
        "ws-l:127.0.0.1:45913",
        noopts,
        nodelay);
    let prog2 = wt!(core,
        "ws://127.0.0.1:45913/ololo",
        "assert:qwert3y",
        noopts,
        delay=200);

    let prog = prog1.join(prog2);
    run!(core, prog);
}

#[test]
fn ws_persist() {
    prepare!(core);
    let prog1 = wt!(core,
        "ws-l:127.0.0.1:45914",
        "literal:qwert4y",
        noopts,
        nodelay);
    let prog2 = wt!(core,
        "literal:invalid_connection_request",
        "tcp:127.0.0.1:45914",
        noopts,
        delay=200 );
    let prog3 = wt!(core,
        "ws://127.0.0.1:45914/ololo",
        "assert:qwert4y",
        noopts,
        delay=400 );

    core.handle().spawn(prog1);
    core.handle().spawn(prog2);
    let prog = prog3;
    run!(core, prog);
}


#[test]
#[cfg(unix)]
fn unix() {
    prepare!(core);
    let prog1 = wt!(core, 
        "literal:qwert3y", 
        "unix-l:zxc",
        opts=Options { unlink_unix_socket: true, ..dflt() },
        nodelay);
    let prog2 = wt!(core, 
        "unix-c:zxc", 
        "assert:qwert3y", 
        noopts, 
        delay=200);

    let prog = prog1.join(prog2);
    run!(core, prog);
    let _ = ::std::fs::remove_file("zxc");
}

#[test]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn abstract_() {
    prepare!(core);
    let prog1 = wt!(core,
        "literal:qwert4y",
        "abstract-l:zxc",
        noopts,
        nodelay);
    let prog2 = wt!(core,
        "abstract-c:zxc",
        "assert:qwert4y",
        noopts,
        delay=200);

    let prog = prog1.join(prog2);
    run!(core, prog);
}
