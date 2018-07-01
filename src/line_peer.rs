use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::Read;
use tokio_io::AsyncRead;

use std::io::Error as IoError;

#[derive(Debug)]
pub struct Message2Line<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Message2Line<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p| packet2line_peer(p))
    }
    specifier_boilerplate!(typ=Line noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = Message2LineClass,
    target = Message2Line,
    prefixes = ["msg2line:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Line filter: ensure each message (a chunk from one read call from underlying specifier)
contains no inner newlines and terminates with one newline.

Reverse of the `line2msg:`.

Replaces both newlines (\x0A) and carrige returns (\x0D) with spaces (\x20) for each read.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option on top of the stack containing a websocket.

Example: TODO
"#
);

#[derive(Debug)]
pub struct Line2Message<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Line2Message<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let retain_newlines = cp.program_options.linemode_retain_newlines;
        let inner = self.0.construct(cp.clone());
        inner.map(move |p| line2packet_peer(p, retain_newlines))
    }
    specifier_boilerplate!(typ=Line noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name=Line2MessageClass, 
    target=Line2Message,
    prefixes=["line2msg:"], 
    arg_handling=subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help=r#"
Line filter: encure that each message (a successful read call) is obtained from a line
coming from underlying specifier, buffering up or splitting content as needed.

Reverse of the `msg2line:`.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option at the top of the stack opposite to websocket-containing stack.

Example: TODO
"#
);

pub fn packet2line_peer(inner_peer: Peer) -> BoxedNewPeerFuture {
    let filtered = Packet2LineWrapper(inner_peer.0);
    let thepeer = Peer::new(filtered, inner_peer.1);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct Packet2LineWrapper(Box<AsyncRead>);

impl Read for Packet2LineWrapper {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let l = b.len();
        assert!(l > 1);
        let mut n = match self.0.read(&mut b[..(l - 1)]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(n);
        }
        // chomp away \n or \r\n
        if n > 0 && b[n - 1] == b'\n' {
            n -= 1;
        }
        if n > 0 && b[n - 1] == b'\r' {
            n -= 1;
        }
        // replace those with spaces
        for c in b.iter_mut().take(n) {
            if *c == b'\n' || *c == b'\r' {
                *c = b' ';
            }
        }
        // add back one \n
        b[n] = b'\n';
        n += 1;

        Ok(n)
    }
}
impl AsyncRead for Packet2LineWrapper {}

pub fn line2packet_peer(inner_peer: Peer, retain_newlines: bool) -> BoxedNewPeerFuture {
    let filtered = Line2PacketWrapper {
        inner: inner_peer.0,
        queue: vec![],
        retain_newlines,
    };
    let thepeer = Peer::new(filtered, inner_peer.1);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct Line2PacketWrapper {
    inner: Box<AsyncRead>,
    queue: Vec<u8>,
    retain_newlines: bool,
}

impl Read for Line2PacketWrapper {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        //eprint!("ql={} ", self.queue.len());
        let mut queued_line_len = None;
        for i in 0..self.queue.len() {
            if self.queue[i] == b'\n' {
                queued_line_len = Some(i);
                break;
            }
        }
        //eprint!("qll={:?} ", queued_line_len);

        if let Some(mut n) = queued_line_len {
            n += 1;
            buf[0..n].copy_from_slice(&self.queue[0..n]);
            ::std::mem::drop(self.queue.drain(0..n));
            if !self.retain_newlines {
                if n > 0 && (buf[n - 1] == b'\n') {
                    n -= 1
                }
                if n > 0 && (buf[n - 1] == b'\r') {
                    n -= 1
                }
            }
            //eprintln!("n={}", n);
            Ok(n)
        } else {
            let mut n = match self.inner.read(buf) {
                Ok(x) => x,
                Err(e) => return Err(e),
            };

            if n == 0 {
                if self.queue.is_empty() {
                    warn!(
                        "Throwing away {} bytes of incomplete line",
                        self.queue.len()
                    );
                }
                return Ok(0);
            }

            let mut happy_case =
                self.queue.is_empty() && (!buf[0..(n - 1)].contains(&b'\n')) && buf[n - 1] == b'\n';

            if happy_case {
                // Specifically to avoid allocations when data is already nice
                if !self.retain_newlines {
                    if n > 0 && (buf[n - 1] == b'\n') {
                        n -= 1
                    }
                    if n > 0 && (buf[n - 1] == b'\r') {
                        n -= 1
                    }
                }
                //eprintln!("happy n={}", n);
                Ok(n)
            } else {
                // Just queue up and recurse
                self.queue.extend_from_slice(&buf[0..n]);
                //eprintln!(" recurse");
                self.read(buf)
            }
        }
    }
}
impl AsyncRead for Line2PacketWrapper {}
