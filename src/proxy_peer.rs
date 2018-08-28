#![allow(unused)]
use futures::future::{err, Future, ok};
use futures::stream::Stream;

use std::cell::RefCell;
use std::rc::Rc;

use options::StaticFile;

use super::readdebt::{DebtHandling, ReadDebt};
use super::{box_up_err, io_other_error, BoxedNewPeerFuture, Peer, simple_err, peer_err, peer_strerr};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use ::tokio_io::io::{write_all,read_exact};



#[derive(Debug)]
pub struct SocksProxy<T: Specifier>(pub T);
impl<T: Specifier> Specifier for SocksProxy<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            connect_socks5_peer(
                p,
                l2r,
            )
        })
    }
    specifier_boilerplate!(typ=WebSocket noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = SocksProxyClass,
    target = SocksProxy,
    prefixes = ["socks5-connect:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
SOCKS5 proxy client (raw) [A]

Example: connect to a websocket using local `ssh -D` proxy

    websocat -t - ws-c:socks5-connect:tcp:127.0.0.1:1080 --socks5-target echo.websocket.org:80 --ws-c-uri ws://echo.websocket.org

For a user-friendly solution, see --socks5 command-line option
"#
);



pub fn connect_socks5_peer(
    inner_peer: Peer,
    l2r: L2rUser,
) -> BoxedNewPeerFuture {
    let (r,w) = (inner_peer.0, inner_peer.1);
    let f = write_all(w, b"\x05\x01\x00").map_err(box_up_err).and_then(move |(w,_)| {
        
        let authmethods = [0; 2];
        read_exact(r, authmethods).map_err(box_up_err).and_then(move |(r, authmethods)| {
            if authmethods[0] != b'\x05' {
                return peer_strerr("Not a SOCKS5 reply");
            }
            if authmethods[1] != b'\x00' {
                return peer_strerr("Not a SOCKS5 or auth required");
            }
            
            //174.129.224.73:80 - echo.websocket.org
            Box::new(write_all(w, b"\x05\x01\x00\x01\xAE\x81\xE0\x49\x00\x50").map_err(box_up_err).and_then(move |(w, _)| {
                
                let reply = [0; 4+4+2];
                
                read_exact(r, reply).map_err(box_up_err).and_then(move |(r, reply)| {
                    
                    if reply[0] != b'\x05' {
                        return peer_strerr("Not a SOCKS5 reply 2");
                    }
                    if reply[1] != b'\x00' {
                        let msg = match reply[1] {
                            1 => "SOCKS: General SOCKS server failuire",
                            2 => "SOCKS connection not allowed",
                            3 => "SOCKS: network unreachable",
                            4 => "SOCKS: host unreachable",
                            5 => "SOCKS: connection refused",
                            6 => "SOCKS: TTL expired",
                            7 => "SOCKS: Command not supported",
                            8 => "SOCKS: Address type not supported",
                            _ => "SOCKS: Unknown failure",
                        };
                        return peer_strerr(msg);
                    }
                    if reply[3] != b'\x01' {
                        return peer_strerr("SOCKS: bound address type is not ipv4");
                    }

                    info!("Connected to SOCKS5 proxy");
                    Box::new(ok(Peer::new(r,w))) as BoxedNewPeerFuture
                })
            })) as BoxedNewPeerFuture
        })
    });
    Box::new(f) as BoxedNewPeerFuture
}
