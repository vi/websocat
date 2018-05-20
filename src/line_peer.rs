#![allow(unused)]

use futures::future::Future;
use futures::future::ok;
use futures::stream::Stream;

use std::cell::RefCell;
use std::rc::Rc;

use super::ws_peer::{Mode1, PeerForWs, WsReadWrapper, WsWriteWrapper};
use super::{box_up_err, io_other_error, BoxedNewPeerFuture, Peer};
use super::{Handle, Options, PeerConstructor, ProgramState, Specifier};

use tokio_io::AsyncRead;
use std::io::Read;

use futures;
use std::io::Error as IoError;

#[derive(Debug)]
pub struct Packet2Line<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Packet2Line<T> {
    fn construct(&self, h: &Handle, ps: &mut ProgramState, opts: Rc<Options>) -> PeerConstructor {
        let inner = self.0.construct(h, ps, opts);
        inner.map(move |p| packet2line_peer(p))
    }
    specifier_boilerplate!(typ=Other noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name=Packet2LineClass, 
    target=Packet2Line,
    prefixes=["packet2line:"], 
    arg_handling=subspec,
    help=r#"
Line filter: ensure each packet (a chunk from one read call) contains no
inner newlines and terminates with one newline.

Reverse of the `line2packet:`.

Replaces both newlines (\x0A) and carrige returns (\x0D) with spaces (\x20) for each read.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

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
        let mut n = match self.0.read(&mut b[..(l-1)]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(n)
        }
        // chomp away \n or \r\n
        if n>0 && b[n-1] == b'\n' {
            n-=1;
        }
        if n>0 && b[n-1] == b'\r' {
            n-=1;
        }
        // replace those with spaces
        for i in 0..n {
            if b[i] == b'\n' || b[i] == b'\r' { 
                b[i] = b' ';
            }
        }
        // add back one \n
        b[n] = b'\n';
        n+=1;
        
        Ok(n)
    }
}
impl AsyncRead for Packet2LineWrapper {
}
