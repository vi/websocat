use futures::future::ok;

use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::Read;
use tokio_io::AsyncRead;

use std::io::Error as IoError;

#[derive(Debug)]
pub struct JsonRpc<T: Specifier>(pub T);
impl<T: Specifier> Specifier for JsonRpc<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _| jsonrpc_peer(p, cp.program_options.jsonrpc_omit_jsonrpc))
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = JsonRpcClass,
    target = JsonRpc,
    prefixes = ["jsonrpc:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Turns messages like `abc 1,2` into `{"jsonrpc":"2.0","id":412, "method":"abc", "params":[1,2]}`.

For simpler manual testing of websocket-based JSON-RPC services

Example: TODO
"#
);

pub fn jsonrpc_peer(inner_peer: Peer, omit_jsonrpc: bool) -> BoxedNewPeerFuture {
    let filtered = JsonRpcWrapper(inner_peer.0, 1, omit_jsonrpc);
    let thepeer = Peer::new(filtered, inner_peer.1, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct JsonRpcWrapper(Box<dyn AsyncRead>, u64, bool);

impl Read for JsonRpcWrapper {
    fn read(&mut self, b: &mut [u8]) -> Result<usize, IoError> {
        let l = b.len();
        assert!(l > 1);
        let n = match self.0.read(&mut b[..l]) {
            Ok(x) => x,
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(0);
        }
        let mut method = Vec::with_capacity(20);
        let mut params = Vec::with_capacity(20);
        enum PS {
            BeforeMethodName,
            InsideMethodName,
            AfterMethodName,
            InsideParams,
        }
        let mut s = PS::BeforeMethodName;
        for &c in b[..n].iter() {
            match s {
                PS::BeforeMethodName => {
                    if c == b' ' || c == b'\t' || c == b'\n' {
                        // ignore
                    } else {
                        method.push(c);
                        s = PS::InsideMethodName;
                    }
                }
                PS::InsideMethodName => {
                    if c == b' ' || c == b'\t' || c == b'\n' {
                        s = PS::AfterMethodName;
                    } else {
                        method.push(c);
                    }
                }
                PS::AfterMethodName => {
                    if c == b' ' || c == b'\t' || c == b'\n' {
                        // ignore
                    } else {
                        params.push(c);
                        s = PS::InsideParams;
                    }
                }
                PS::InsideParams => {
                    params.push(c);
                }
            }
        }

        let mut bb = ::std::io::Cursor::new(b);
        use std::io::Write;
        //{"jsonrpc":"2.0","id":412, "method":"abc", "params":[1,2]}
        if self.2 {
            let _ = bb.write_all(b"{\"id\":");
        } else {
            let _ = bb.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":");
        }
        let _ = bb.write_all(format!("{}", self.1).as_bytes());
        self.1 += 1;
        let _ = bb.write_all(b", \"method\":\"");
        let _ = bb.write_all(&method);
        let _ = bb.write_all(b"\", \"params\":");
        let needs_brackets = params.is_empty() || params[0] != b'{' && params[0] != b'[';
        if !params.is_empty() && params[params.len() - 1] == b'\n' {
            let l = params.len() - 1;
            params.truncate(l);
        }
        if !params.is_empty() && params[params.len() - 1] == b'\r' {
            let l = params.len() - 1;
            params.truncate(l);
        }
        if needs_brackets {
            let _ = bb.write_all(b"[");
        }
        let _ = bb.write_all(&params);
        if needs_brackets {
            let _ = bb.write_all(b"]");
        }
        let _ = bb.write_all(b"}\n");
        if bb.position() as usize == l {
            warn!("Buffer too small, JSON RPC message may be truncated.");
        }
        Ok(bb.position() as usize)
    }
}
impl AsyncRead for JsonRpcWrapper {}
