extern crate websocat;

extern crate env_logger;
extern crate futures;
extern crate tokio;
extern crate tokio_timer;

use futures::future::Future;

use websocat::{spec, Options, WebsocatConfiguration3};

fn dflt() -> Options {
    Default::default()
}

macro_rules! wt {
    ($core:ident, $s1:expr, $s2:expr,delay = $ms:expr, $($rest:tt)*) => {{
        let s1 = spec($s1).unwrap();
        let s2 = spec($s2).unwrap();

        let delay = tokio_timer::Delay::new(std::time::Instant::now() + std::time::Duration::new(0, $ms * 1_000_000)).map_err(|_|());

        delay.and_then(|()| wt!(stage2, h2, s1, s2, $($rest)*) )
    }};
    ($core:ident, $s1:expr, $s2:expr,nodelay,$($rest:tt)*) => {{
        let s1 = spec($s1).unwrap();
        let s2 = spec($s2).unwrap();

        wt!(stage2, h2, s1, s2, $($rest)*)
    }};
    (stage2, $h2:ident, $s1:ident, $s2:ident, noopts,$($rest:tt)*) => {
        wt!(stage2, $h2, $s1, $s2, opts=Default::default(),$($rest)*)
    };
    (stage2, $h2:ident, $s1:ident, $s2:ident, opts=$opts:expr,$($rest:tt)*) => {{

        let websocat = WebsocatConfiguration3 {
            opts: $opts,
            $s1,
            $s2,
        };

        websocat.serve(
            wt!(stage3, $($rest)*),
        )
    }};
    (stage3, errpanic,) => {
        std::rc::Rc::new(|e| {
            eprintln!("{}", e);
            panic!();
        })
    };
    (stage3, errignore,) => {
        std::rc::Rc::new(|_| {

        })
    };
}

macro_rules! prepare {
    ($core:ident) => {
        let _ = env_logger::try_init();
        let mut $core = tokio::runtime::current_thread::Runtime::new().unwrap();
    };
}
macro_rules! run {
    ($core:ident, $prog:expr) => {
        $core.block_on($prog).map_err(|()| panic!()).unwrap();
    };
}

#[test]
fn trivial() {
    prepare!(core);
    let prog = wt!(
        core,
        "literal:qwerty",
        "assert:qwerty",
        nodelay,
        noopts,
        errpanic,
    );
    run!(core, prog);
}

#[test]
fn tcp() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "literal:qwert2y",
        "tcp-l:127.0.0.1:45912",
        nodelay,
        noopts,
        errpanic,
    );
    let prog2 = wt!(
        core,
        "tcp:127.0.0.1:45912",
        "assert:qwert2y",
        delay = 200,
        noopts,
        errpanic,
    );

    let prog = prog1.join(prog2);
    run!(core, prog);
}

#[test]
fn ws() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "literal:qwert3y",
        "ws-l:127.0.0.1:45913",
        nodelay,
        noopts,
        errpanic,
    );
    let prog2 = wt!(
        core,
        "ws://127.0.0.1:45913/ololo",
        "assert:qwert3y",
        delay = 200,
        noopts,
        errpanic,
    );

    let prog = prog1.join(prog2);
    run!(core, prog);
}

#[test]
fn ws_ll() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "literal:qwert3y",
        "ws-ll-s:tcp-l:127.0.0.1:45915",
        nodelay,
        noopts,
        errpanic,
    );
    let prog2 = wt!(
        core,
        "ws-ll-c:tcp:127.0.0.1:45915",
        "assert:qwert3y",
        delay = 200,
        noopts,
        errpanic,
    );

    let prog = prog1.join(prog2);
    run!(core, prog);
}


#[test]
fn ws_persist() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "ws-l:127.0.0.1:45914",
        "literal:qwert4y",
        nodelay,
        noopts,
        errignore,
    );
    let prog2 = wt!(
        core,
        "literal:invalid_connection_request",
        "tcp:127.0.0.1:45914",
        delay = 200,
        noopts,
        errpanic,
    );
    let prog3 = wt!(
        core,
        "ws://127.0.0.1:45914/ololo",
        "assert:qwert4y",
        delay = 400,
        noopts,
        errpanic,
    );

    core.spawn(prog1);
    core.spawn(prog2);
    let prog = prog3;
    run!(core, prog);
}

#[test]
#[cfg(unix)]
fn unix() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "literal:qwert3y",
        "unix-l:zxc",
        nodelay,
        opts = Options {
            unlink_unix_socket: true,
            ..dflt()
        },
        errpanic,
    );
    let prog2 = wt!(
        core,
        "unix-c:zxc",
        "assert:qwert3y",
        delay = 200,
        noopts,
        errpanic,
    );

    let prog = prog1.join(prog2);
    run!(core, prog);
    let _ = ::std::fs::remove_file("zxc");
}

#[test]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn abstract_() {
    prepare!(core);
    let prog1 = wt!(
        core,
        "literal:qwert4y",
        "abstract-l:zxc",
        nodelay,
        noopts,
        errpanic,
    );
    let prog2 = wt!(
        core,
        "abstract-c:zxc",
        "assert:qwert4y",
        delay = 200,
        noopts,
        errpanic,
    );

    let prog = prog1.join(prog2);
    run!(core, prog);
}
