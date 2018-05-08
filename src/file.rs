use std;
use futures;
use futures::Async;
use std::path::{PathBuf,Path};
use tokio_io::{AsyncRead,AsyncWrite};
use std::io::{Read,Write};
use std::io::Result as IoResult;

#[cfg(unix)]
use ::std::fs::{File};

use super::{Peer, BoxedNewPeerFuture, Result};


use super::{once,Handle,Specifier,ProgramState,PeerConstructor,Options};

#[derive(Clone,Debug)]
pub struct ReadFile(pub PathBuf);
impl Specifier for ReadFile {
    fn construct(&self, _h:&Handle, _ps: &mut ProgramState, _opts: &Options) -> PeerConstructor {
        fn gp(p : &Path) -> Result<Peer> {
            let f = File::open(p)?;
            Ok(Peer::new(ReadFileWrapper(f), super::trivial_peer::DevNull))
        }
        once(Box::new( futures::future::result(gp(&self.0)) ) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(typ=Other noglobalstate singleconnect no_subspec);
}

#[derive(Clone,Debug)]
pub struct WriteFile(pub PathBuf);
impl Specifier for WriteFile {
    fn construct(&self, _h:&Handle, _ps: &mut ProgramState, _opts: &Options) -> PeerConstructor {
        fn gp(p : &Path) -> Result<Peer> {
            let f = File::create(p)?;
            Ok(Peer::new(super::trivial_peer::DevNull, WriteFileWrapper(f)))
        }
        once(Box::new( futures::future::result(gp(&self.0)) ) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(typ=Other noglobalstate singleconnect no_subspec);
}

struct ReadFileWrapper(File);

impl AsyncRead for ReadFileWrapper{}
impl Read for ReadFileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.0.read(buf)
    }
}

struct WriteFileWrapper(File);

impl AsyncWrite for WriteFileWrapper {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        Ok(Async::Ready(()))
    }
}
impl Write for WriteFileWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}
