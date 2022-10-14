#![cfg_attr(feature="cargo-clippy",allow(needless_pass_by_value,cast_lossless,identity_op))]
use futures::future::{err, ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_strerr, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use tokio_io::io::{read_exact, write_all};

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

use std::ffi::OsString;

#[derive(Debug, Clone)]
pub enum SocksHostAddr {
    Ip(IpAddr),
    Name(String),
}

#[derive(Debug, Clone)]
pub struct SocksSocketAddr {
    pub host: SocksHostAddr,
    pub port: u16,
}

#[derive(Debug)]
pub struct SocksProxy<T: Specifier>(pub T);
impl<T: Specifier> Specifier for SocksProxy<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            socks5_peer(p, l2r, false, None, &cp.program_options.socks_destination, false)
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
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

    websocat -t - ws-c:socks5-connect:tcp:127.0.0.1:1080 --socks5-destination echo.websocket.org:80 --ws-c-uri ws://echo.websocket.org

For a user-friendly solution, see --socks5 command-line option
"#
);

#[derive(Debug)]
pub struct SocksBind<T: Specifier>(pub T);
impl<T: Specifier> Specifier for SocksBind<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            socks5_peer(
                p,
                l2r,
                true,
                cp.program_options.socks5_bind_script.clone(),
                &cp.program_options.socks_destination,
                cp.program_options.announce_listens,
            )
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = SocksBindClass,
    target = SocksBind,
    prefixes = ["socks5-bind:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
SOCKS5 proxy client (raw, bind command) [A]

Example: bind to a websocket using some remote SOCKS server

    websocat -v -t ws-u:socks5-bind:tcp:132.148.129.183:14124 - --socks5-destination 255.255.255.255:65535

Note that port is typically unpredictable. Use --socks5-bind-script option to know the port.
See an example in moreexamples.md for more thorough example.
"#
);

type RSRRet =
    Box<dyn Future<Item = (SocksSocketAddr, Peer), Error = Box<dyn (::std::error::Error)>>>;
fn read_socks_reply(p: Peer) -> RSRRet {
    let (r, w, hup) = (p.0, p.1, p.2);
    let reply = [0; 4];

    fn myerr(x: &'static str) -> RSRRet {
        Box::new(err(x.to_string().into()))
    }

    Box::new(
        read_exact(r, reply)
            .map_err(box_up_err)
            .and_then(move |(r, reply)| {
                if reply[0] != b'\x05' {
                    return myerr("Not a SOCKS5 reply 2");
                }
                if reply[1] != b'\x00' {
                    let msg = match reply[1] {
                        1 => "SOCKS: General SOCKS server failure",
                        2 => "SOCKS connection not allowed",
                        3 => "SOCKS: network unreachable",
                        4 => "SOCKS: host unreachable",
                        5 => "SOCKS: connection refused",
                        6 => "SOCKS: TTL expired",
                        7 => "SOCKS: Command not supported",
                        8 => "SOCKS: Address type not supported",
                        _ => "SOCKS: Unknown failure",
                    };
                    return myerr(msg);
                }
                let ret: RSRRet;
                ret = match reply[3] {
                    b'\x01' => {
                        // ipv4
                        let addrport = [0; 4 + 2];
                        Box::new(read_exact(r, addrport).map_err(box_up_err).and_then(
                            move |(r, addrport)| {
                                let port = (addrport[4] as u16) * 256 + (addrport[5] as u16);
                                let ip = Ipv4Addr::new(
                                    addrport[0],
                                    addrport[1],
                                    addrport[2],
                                    addrport[3],
                                );
                                let host = SocksHostAddr::Ip(IpAddr::V4(ip));
                                ok((SocksSocketAddr { host, port }, Peer(r, w, hup)))
                            },
                        ))
                    }
                    b'\x04' => {
                        // ipv6
                        let addrport = [0; 16 + 2];
                        Box::new(read_exact(r, addrport).map_err(box_up_err).and_then(
                            move |(r, addrport)| {
                                let port = (addrport[16] as u16) * 256 + (addrport[17] as u16);
                                // still not worth to switch to Cargo.toml to add
                                // "bytes" dependency, then scroll up for "extern crate",
                                // then look up docs again to find out where to get that BE thing.
                                let mut ip = [0u8; 16];
                                ip.copy_from_slice(&addrport[0..16]);
                                let host = SocksHostAddr::Ip(IpAddr::V6(ip.into()));
                                ok((SocksSocketAddr { host, port }, Peer(r, w, hup)))
                            },
                        ))
                    }
                    b'\x03' => {
                        let alen = [0; 1];
                        Box::new(read_exact(r, alen).map_err(box_up_err).and_then(
                            move |(r, alen)| {
                                let alen = alen[0] as usize;
                                let addrport = vec![0; alen + 2];

                                read_exact(r, addrport).map_err(box_up_err).and_then(
                                    move |(r, addrport)| {
                                        let port = (addrport[alen] as u16) * 256
                                            + (addrport[alen + 1] as u16);
                                        let host = SocksHostAddr::Name(
                                            ::std::str::from_utf8(&addrport[0..alen])
                                                .unwrap_or("(invalid hostname)")
                                                .to_string(),
                                        );
                                        ok((SocksSocketAddr { host, port }, Peer(r, w, hup)))
                                    },
                                )
                            },
                        ))
                    }
                    _ => {
                        return myerr("SOCKS: bound address type is unknown");
                    }
                };
                ret
            }),
    )
}

pub fn socks5_peer(
    inner_peer: Peer,
    _l2r: L2rUser,
    do_bind: bool,
    bind_script: Option<OsString>,
    socks_destination: &Option<SocksSocketAddr>,
    announce_listen: bool,
) -> BoxedNewPeerFuture {
    let (desthost, destport) = if let Some(ref sd) = *socks_destination {
        (sd.host.clone(), sd.port)
    } else {
        return peer_strerr(
            "--socks5-destination is required for socks5-connect: or socks5-bind: overlays",
        );
    };

    if let SocksHostAddr::Name(ref n) = desthost {
        if n.len() > 255 {
            return peer_strerr("Destination host name too long for SOCKS5");
        }
    };

    info!("Connecting to SOCKS server");
    let (r, w, hup) = (inner_peer.0, inner_peer.1, inner_peer.2);
    let f = write_all(w, b"\x05\x01\x00")
        .map_err(box_up_err)
        .and_then(move |(w, _)| {
            let authmethods = [0; 2];
            read_exact(r, authmethods)
                .map_err(box_up_err)
                .and_then(move |(r, authmethods)| {
                    if authmethods[0] != b'\x05' {
                        return peer_strerr("Not a SOCKS5 reply");
                    }
                    if authmethods[1] != b'\x00' {
                        return peer_strerr("Not a SOCKS5 or auth required");
                    }

                    let rq = {
                        let mut c = ::std::io::Cursor::new(Vec::with_capacity(20));
                        if do_bind {
                            c.write_all(b"\x05\x02\x00").unwrap();
                        } else {
                            c.write_all(b"\x05\x01\x00").unwrap();
                        };
                        match desthost {
                            SocksHostAddr::Ip(IpAddr::V4(ip4)) => {
                                c.write_all(b"\x01").unwrap();
                                c.write_all(&ip4.octets()).unwrap();
                            }
                            SocksHostAddr::Ip(IpAddr::V6(ip6)) => {
                                c.write_all(b"\x04").unwrap();
                                c.write_all(&ip6.octets()).unwrap();
                            }
                            SocksHostAddr::Name(name) => {
                                c.write_all(b"\x03").unwrap();
                                c.write_all(&[name.len() as u8]).unwrap();
                                c.write_all(name.as_bytes()).unwrap();
                            }
                        };
                        c.write_all(&[(destport >> 8) as u8]).unwrap();
                        c.write_all(&[(destport >> 0) as u8]).unwrap();
                        c.into_inner()
                    };

                    Box::new(
                        write_all(w, rq)
                            .map_err(box_up_err)
                            .and_then(move |(w, _)| {
                                let _reply = [0; 4];

                                read_socks_reply(Peer(r, w, hup)).and_then(move |(addr, p)| {
                                    info!("SOCKS5 connect/bind: {:?}", addr);

                                    if do_bind {
                                        if announce_listen {
                                            println!("LISTEN proto=tcp,port={}", addr.port);
                                        }
                                        if let Some(bs) = bind_script {
                                            let _ = ::std::process::Command::new(bs)
                                                .arg(format!("{}", addr.port))
                                                .spawn();
                                        }

                                        Box::new(read_socks_reply(p).and_then(move |(addr, p)| {
                                            info!("SOCKS5 remote connected: {:?}", addr);
                                            Box::new(ok(p))
                                        }))
                                            as BoxedNewPeerFuture
                                    } else {
                                        Box::new(ok(p)) as BoxedNewPeerFuture
                                    }
                                })
                            }),
                    ) as BoxedNewPeerFuture
                })
        });
    Box::new(f) as BoxedNewPeerFuture
}
