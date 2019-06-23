use futures;
use futures::Async;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio_io::{AsyncRead, AsyncWrite};

use std::fs::{File, OpenOptions};
use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer, Result};

use super::{once, ConstructParams, PeerConstructor, Specifier};

#[derive(Clone, Debug)]
pub struct ReadFile(pub PathBuf);
impl Specifier for ReadFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = File::open(p)?;
            Ok(Peer::new(ReadFileWrapper(f), super::trivial_peer::DevNull, None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = ReadFileClass,
    target = ReadFile,
    prefixes = ["readfile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Synchronously read a file. Argument is a file path.

Blocking on operations with the file pauses the whole process

Example: Serve the file once per connection, ignore all replies.

    websocat ws-l:127.0.0.1:8000 readfile:hello.json

"#
);

#[derive(Clone, Debug)]
pub struct WriteFile(pub PathBuf);
impl Specifier for WriteFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = File::create(p)?;
            Ok(Peer::new(super::trivial_peer::DevNull, WriteFileWrapper(f), None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = WriteFileClass,
    target = WriteFile,
    prefixes = ["writefile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"

Synchronously truncate and write a file.

Blocking on operations with the file pauses the whole process

Example:

    websocat ws-l:127.0.0.1:8000 writefile:data.txt

"#
);

#[derive(Clone, Debug)]
pub struct AppendFile(pub PathBuf);
impl Specifier for AppendFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = OpenOptions::new().create(true).append(true).open(p)?;
            Ok(Peer::new(super::trivial_peer::DevNull, WriteFileWrapper(f), None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = AppendFileClass,
    target = AppendFile,
    prefixes = ["appendfile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"

Synchronously append a file.

Blocking on operations with the file pauses the whole process

Example: Logging all incoming data from WebSocket clients to one file

    websocat -u ws-l:127.0.0.1:8000 reuse:appendfile:log.txt
"#
);

pub struct ReadFileWrapper(pub File);

impl AsyncRead for ReadFileWrapper {}
impl Read for ReadFileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.0.read(buf)
    }
}

struct WriteFileWrapper(File);

impl AsyncWrite for WriteFileWrapper {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
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
