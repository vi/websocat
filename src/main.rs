#![allow(unused_extern_crates,unused_imports)]

//extern crate websocket;
//extern crate env_logger;
//#[macro_use]
//extern crate log;
//extern crate url;
//#[macro_use] // crate_version
//extern crate clap;

extern crate tokio_core;
extern crate tokio_io;
#[macro_use]
extern crate futures;

const BUFSIZ: usize = 8192;

use std::io::{Error, ErrorKind, Result, Read, Write};
use futures::{Stream,Poll,Async,Sink,Future,AsyncSink};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead,AsyncWrite};

type BBR = futures::sync::mpsc::Receiver <Box<[u8]>>;
type BBS = futures::sync::mpsc::Sender   <Box<[u8]>>;

struct ThreadedStdin {
    debt : Option<Box<[u8]>>,
    rcv : BBR,
}

impl ThreadedStdin {
    fn new() -> Self {
        let (snd_, rcv) : (BBS,BBR) =  futures::sync::mpsc::channel(0);
        std::thread::spawn(move || {
            let mut snd = snd_;
            let sin = ::std::io::stdin();
            let mut sin_lock = sin.lock();
            let mut buf = vec![0; BUFSIZ];
            loop {
                let ret = match sin_lock.read(&mut buf[..]) {
                    Ok(x) => x,
                    Err(_) => {
                        // BrokenPipe
                        break;
                    }
                };
                let content = buf[0..ret].to_vec().into_boxed_slice();
                snd = match snd.send(content).wait() {
                    Ok(x) => x,
                    Err(_) => break,
                }
            }
        });
        ThreadedStdin {
            debt: None,
            rcv,
        }
    }
}

impl AsyncRead for ThreadedStdin {}
impl Read for ThreadedStdin {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    
        let mut handle_the_buffer = |incoming_buf:Box<[u8]>| {
            let l = buf.len();
            let dl = incoming_buf.len();
            if l >= dl {
                buf[0..dl].copy_from_slice(&incoming_buf);
                (None, Ok(dl))
            } else {
                buf[0..l].copy_from_slice(&incoming_buf[0..l]);
                let newdebt = Some(incoming_buf[l..].to_vec().into_boxed_slice());
                (newdebt, Ok(l))
            }
        };
        
        let (new_debt, ret) =
            if let Some(debt) = self.debt.take() {
                handle_the_buffer(debt)
            } else {
                match self.rcv.poll() {
                    Ok(Async::Ready(Some(newbuf))) => handle_the_buffer(newbuf),
                    Ok(Async::Ready(None)) => (None, Err(ErrorKind::BrokenPipe.into())),
                    Ok(Async::NotReady)    => (None, Err(ErrorKind::WouldBlock.into())),
                    Err(_)                 => (None, Err(ErrorKind::Other.into())),
                }
            };
        self.debt = new_debt;
        return ret
    }
}



struct ThreadedStdout {
    snd : BBS,
}
impl ThreadedStdout {
    fn new() -> Self {
        let (snd, rcv) : (BBS,BBR) =  futures::sync::mpsc::channel(0);
        std::thread::spawn(move || {
            let sout = ::std::io::stdout();
            let mut sout_lock = sout.lock();
            for b in rcv.wait() {
                if let Err(_) = b {
                    break;
                }
                if let Err(_) = sout_lock.write_all(&b.unwrap()) {
                    break;
                }
            }
        });
        ThreadedStdout {
            snd,
        }
    }
}
impl AsyncWrite for ThreadedStdout {
    fn shutdown(&mut self) -> Poll<(), Error> {
        // XXX
        Ok(Async::Ready(()))
    }
}
impl Write for ThreadedStdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self.snd.start_send(buf.to_vec().into_boxed_slice()) {
            Ok(AsyncSink::Ready)       => (),
            Ok(AsyncSink::NotReady(_)) => return Err(ErrorKind::WouldBlock.into()),
            Err(_)                     => return Err(ErrorKind::Other.into()),
        }
        match self.snd.poll_complete() {
            // XXX
            Ok(Async::Ready(_))    => (), // OK
            Ok(Async::NotReady)    => (), // don't know what to do here
            Err(_) => return Err(ErrorKind::Other.into()),
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<()> {
        // XXX
        Ok(())
    }
}


fn run() -> Result<()> {
    let mut core = Core::new()?;
    let handle = core.handle();
    
    let stdin = ThreadedStdin::new();
    let stdout = ThreadedStdout::new();
    core.run(tokio_io::io::copy(stdin, stdout))?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Something failed: {}", e);
        ::std::process::exit(1);
    }
}
