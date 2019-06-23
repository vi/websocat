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
        let zt = cp.program_options.linemode_zero_terminated;
        inner.map(move |p, _| packet2line_peer(p, zt))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
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
Line filter: Turns messages from packet stream into lines of byte stream. [A]

Ensure each message (a chunk from one read call from underlying connection)
contains no inner newlines (or zero bytes) and terminates with one newline.

Reverse of the `line2msg:`.

Unless --null-terminated, replaces both newlines (\x0A) and carriage returns (\x0D) with spaces (\x20) for each read.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option on top of the stack containing a websocket.

Example: TODO
"#
);

#[derive(Debug)]
pub struct Line2Message<T: Specifier>(pub T);
impl<T: Specifier> Specifier for Line2Message<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let retain_newlines = !cp.program_options.linemode_strip_newlines;
        let strict = cp.program_options.linemode_strict;
        let nullt = cp.program_options.linemode_zero_terminated;
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _| line2packet_peer(p, retain_newlines, strict, nullt))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
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
Line filter: turn lines from byte stream into messages as delimited by '\\n' or '\\0' [A]

Ensure that each message (a successful read call) is obtained from a line [A]
coming from underlying specifier, buffering up or splitting content as needed.

Reverse of the `msg2line:`.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option at the top of the stack opposite to websocket-containing stack.

Example: TODO
"#
);

pub fn packet2line_peer(inner_peer: Peer, null_terminated: bool) -> BoxedNewPeerFuture {
    let filtered = Packet2LineWrapper(inner_peer.0, null_terminated);
    let thepeer = Peer::new(filtered, inner_peer.1, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct Packet2LineWrapper(Box<dyn AsyncRead>, bool);

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
        if !self.1 {
            // newline-terminated

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
        } else {
            // null-terminated
            if n > 0 && b[n - 1] == b'\x00' {
                n -= 1;
            }
            for c in b.iter_mut().take(n) {
                if *c == b'\x00' {
                    warn!("zero byte in a message in null-terminated mode");
                }
            }
            b[n] = b'\x00';
            n += 1;
        }

        Ok(n)
    }
}
impl AsyncRead for Packet2LineWrapper {}

pub fn line2packet_peer(
    inner_peer: Peer,
    retain_newlines: bool,
    strict: bool,
    null_terminated: bool,
) -> BoxedNewPeerFuture {
    let filtered = Line2PacketWrapper {
        inner: inner_peer.0,
        queue: vec![],
        retain_newlines,
        allow_incomplete_lines: !strict,
        drop_too_long_lines: strict,
        eof: false,
        null_terminated,
    };
    let thepeer = Peer::new(filtered, inner_peer.1, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct Line2PacketWrapper {
    inner: Box<dyn AsyncRead>,
    queue: Vec<u8>,
    retain_newlines: bool,
    allow_incomplete_lines: bool,
    drop_too_long_lines: bool,
    eof: bool,
    null_terminated: bool,
}

impl Line2PacketWrapper {
    #[cfg_attr(feature = "cargo-clippy", allow(collapsible_if))]
    fn deliver_the_line(&mut self, buf: &mut [u8], mut n: usize) -> Option<usize> {
        if n > buf.len() {
            if self.drop_too_long_lines {
                error!("Dropping too long line of {} bytes because of buffer (-B option) is only {} bytes", n, buf.len());
                drop(self.queue.drain(0..n));
                return None;
            } else {
                warn!("Splitting too long line of {} bytes because of buffer (-B option) is only {} bytes", n, buf.len());
                n = buf.len();
            }
        } else {
            if !self.retain_newlines && !self.null_terminated {
                if n > 0 && (buf[n - 1] == b'\n') {
                    n -= 1
                }
                if n > 0 && (buf[n - 1] == b'\r') {
                    n -= 1
                }
            }
            if self.null_terminated {
                if n > 0 && (buf[n - 1] == b'\x00') {
                    n -= 1
                }
            }
        }

        buf[0..n].copy_from_slice(&self.queue[0..n]);
        drop(self.queue.drain(0..n));
        Some(n)
    }
}

impl Read for Line2PacketWrapper {
    #[cfg_attr(feature = "cargo-clippy", allow(collapsible_if))]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        //eprint!("ql={} ", self.queue.len());
        if self.eof {
            return Ok(0);
        }

        let char_to_look_at = if self.null_terminated { b'\x00' } else { b'\n' };
        let mut queued_line_len = None;
        for i in 0..self.queue.len() {
            if self.queue[i] == char_to_look_at {
                queued_line_len = Some(i);
                break;
            }
        }
        //eprint!("qll={:?} ", queued_line_len);

        if let Some(mut n) = queued_line_len {
            n += 1;
            if let Some(nn) = self.deliver_the_line(buf, n) {
                Ok(nn)
            } else {
                // line dropped, recursing
                self.read(buf)
            }
        } else {
            let mut n = match self.inner.read(buf) {
                Ok(x) => x,
                Err(e) => return Err(e),
            };

            if n == 0 {
                self.eof = true;
                if !self.queue.is_empty() {
                    if self.allow_incomplete_lines {
                        warn!("Sending possibly incomplete line.");
                        let bl = self.queue.len();
                        if let Some(nn) = self.deliver_the_line(buf, bl) {
                            return Ok(nn);
                        }
                    } else {
                        warn!(
                            "Throwing away {} bytes of incomplete line",
                            self.queue.len()
                        );
                    }
                }
                return Ok(0);
            }

            let happy_case = if !self.null_terminated {
                self.queue.is_empty() && (!buf[0..(n - 1)].contains(&b'\n')) && buf[n - 1] == b'\n'
            } else {
                self.queue.is_empty()
                    && (!buf[0..(n - 1)].contains(&b'\x00'))
                    && buf[n - 1] == b'\x00'
            };

            if happy_case {
                // Specifically to avoid allocations when data is already nice
                if !self.retain_newlines && !self.null_terminated {
                    if n > 0 && (buf[n - 1] == b'\n') {
                        n -= 1
                    }
                    if n > 0 && (buf[n - 1] == b'\r') {
                        n -= 1
                    }
                }
                if self.null_terminated {
                    if n > 0 && (buf[n - 1] == b'\x00') {
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
