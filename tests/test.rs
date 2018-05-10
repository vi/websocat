extern crate websocat;

extern crate env_logger;
extern crate futures;
extern crate tokio_core;
extern crate tokio_timer;

use futures::future::Future;

use tokio_core::reactor::Core;
use websocat::{spec, WebsocatConfiguration};

#[test]
fn trivial() {
    let _ = env_logger::try_init();
    let mut core = Core::new().unwrap();

    let s1 = spec("literal:qwerty").unwrap();
    let s2 = spec("assert:qwerty").unwrap();
    let websocat = WebsocatConfiguration {
        opts: Default::default(),
        s1,
        s2,
    };

    let prog = websocat.serve(
        core.handle(),
        std::rc::Rc::new(|_| {
            panic!();
        }),
    );

    core.run(prog).map_err(|()| panic!()).unwrap();
}

#[test]
fn tcp() {
    let _ = env_logger::try_init();
    let mut core = Core::new().unwrap();

    let prog1 = {
        let s1 = spec("literal:qwert2y").unwrap();
        let s2 = spec("tcp-l:127.0.0.1:45912").unwrap();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        websocat.serve(
            core.handle(),
            std::rc::Rc::new(|e| {
                eprintln!("{}", e);
                panic!();
            }),
        )
    };
    let prog2 = {
        let s1 = spec("tcp:127.0.0.1:45912").unwrap();
        let s2 = spec("assert:qwert2y").unwrap();
        let h2 = core.handle();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        let t = tokio_timer::wheel().build();
        let delay = t.sleep(std::time::Duration::new(0, 200_000_000))
            .map_err(|_| ());

        delay.and_then(|()| {
            websocat.serve(
                h2,
                std::rc::Rc::new(|e| {
                    eprintln!("{}", e);
                    panic!();
                }),
            )
        })
    };

    let prog = prog1.join(prog2);
    core.run(prog).map_err(|()| panic!()).unwrap();
}

#[test]
fn ws() {
    let _ = env_logger::try_init();
    let mut core = Core::new().unwrap();

    let prog1 = {
        let s1 = spec("literal:qwert3y").unwrap();
        let s2 = spec("ws-l:127.0.0.1:45913").unwrap();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        websocat.serve(
            core.handle(),
            std::rc::Rc::new(|e| {
                eprintln!("{}", e);
                panic!();
            }),
        )
    };
    let prog2 = {
        let s1 = spec("ws://127.0.0.1:45913/ololo").unwrap();
        let s2 = spec("assert:qwert3y").unwrap();
        let h2 = core.handle();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        let t = tokio_timer::wheel().build();
        let delay = t.sleep(std::time::Duration::new(0, 200_000_000))
            .map_err(|_| ());

        delay.and_then(|()| {
            websocat.serve(
                h2,
                std::rc::Rc::new(|e| {
                    eprintln!("{}", e);
                    panic!();
                }),
            )
        })
    };

    let prog = prog1.join(prog2);
    core.run(prog).map_err(|()| panic!()).unwrap();
}

#[test]
#[cfg(unix)]
fn unix() {
    let _ = env_logger::try_init();
    let mut core = Core::new().unwrap();

    let prog1 = {
        let s1 = spec("literal:qwert3y").unwrap();
        let s2 = spec("unix-l:zxc").unwrap();

        let mut opts: websocat::Options = Default::default();
        opts.unlink_unix_socket = true;
        let websocat = WebsocatConfiguration { opts, s1, s2 };

        websocat.serve(
            core.handle(),
            std::rc::Rc::new(|e| {
                eprintln!("{}", e);
                panic!();
            }),
        )
    };
    let prog2 = {
        let s1 = spec("unix-c:zxc").unwrap();
        let s2 = spec("assert:qwert3y").unwrap();
        let h2 = core.handle();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        let t = tokio_timer::wheel().build();
        let delay = t.sleep(std::time::Duration::new(0, 200_000_000))
            .map_err(|_| ());

        delay.and_then(|()| {
            websocat.serve(
                h2,
                std::rc::Rc::new(|e| {
                    eprintln!("{}", e);
                    panic!();
                }),
            )
        })
    };

    let prog = prog1.join(prog2);
    core.run(prog).map_err(|()| panic!()).unwrap();
    let _ = ::std::fs::remove_file("zxc");
}

#[test]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn abstract_() {
    let _ = env_logger::try_init();
    let mut core = Core::new().unwrap();

    let prog1 = {
        let s1 = spec("literal:qwert4y").unwrap();
        let s2 = spec("abstract-l:zxc").unwrap();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        websocat.serve(
            core.handle(),
            std::rc::Rc::new(|e| {
                eprintln!("{}", e);
                panic!();
            }),
        )
    };
    let prog2 = {
        let s1 = spec("abstract-c:zxc").unwrap();
        let s2 = spec("assert:qwert4y").unwrap();
        let h2 = core.handle();

        let websocat = WebsocatConfiguration {
            opts: Default::default(),
            s1,
            s2,
        };

        let t = tokio_timer::wheel().build();
        let delay = t.sleep(std::time::Duration::new(0, 200_000_000))
            .map_err(|_| ());

        delay.and_then(|()| {
            websocat.serve(
                h2,
                std::rc::Rc::new(|e| {
                    eprintln!("{}", e);
                    panic!();
                }),
            )
        })
    };

    let prog = prog1.join(prog2);
    core.run(prog).map_err(|()| panic!()).unwrap();
}
