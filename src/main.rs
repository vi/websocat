#![allow(unused)]

extern crate websocat;

extern crate websocket;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate tokio_io;
extern crate tokio_stdin_stdout;

use std::thread;
use std::io::stdin;
use tokio_core::reactor::{Core, Handle};
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;
use websocat::{Session};

use std::rc::Rc;
use std::cell::RefCell;

use websocket::stream::async::Stream as WsStream;
use futures::Async::{Ready, NotReady};

use tokio_io::io::copy;

use tokio_io::codec::FramedRead;
use std::fs::File;

#[cfg(unix)]
use std::os::unix::io::FromRawFd;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;



fn run() -> Result<()> {
    let _        = std::env::args().nth(1).ok_or("Usage: websocat - ws[s]://...")?;
    let peeraddr = std::env::args().nth(2).ok_or("no second arg")?;

    //println!("Connecting to {}", peeraddr);
    let mut core = Core::new()?;
    let handle = core.handle();

    let h1 = core.handle();
    let h2 = core.handle();

    let runner = websocat::ws_peer::get_ws_client_peer(&h1, peeraddr.as_ref())
    .and_then(|ws_peer| {
        websocat::stdio_peer::get_stdio_peer(&h2)
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
