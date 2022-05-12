extern crate net2;

use futures;
use futures::future::Future;
use futures::stream::Stream;
use futures::unsync::oneshot::{channel, Receiver, Sender};
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::net::SocketAddr;
use tokio_io::{AsyncRead, AsyncWrite};

use std::cell::RefCell;
use std::rc::Rc;

use tokio_tcp::{TcpListener, TcpStream};
use tokio_udp::UdpSocket;

use super::L2rUser;
use super::{box_up_err, peer_err_s, wouldblock, BoxedNewPeerFuture, BoxedNewPeerStream, Peer};
use super::{multi, once, ConstructParams, Options, PeerConstructor, Specifier};

#[derive(Debug, Clone)]
pub struct TcpConnect(pub Vec<SocketAddr>);
impl Specifier for TcpConnect {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        // FIXME: connect to multiple things
        once(tcp_connect_peer(&self.0[..]))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = TcpConnectClass,
    target = TcpConnect,
    prefixes = ["tcp:", "tcp-connect:", "connect-tcp:", "tcp-c:", "c-tcp:"],
    arg_handling = parseresolve,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Connect to specified TCP host and port. Argument is a socket address.

Example: simulate netcat netcat

    websocat - tcp:127.0.0.1:22

Example: redirect websocket connections to local SSH server over IPv6

    websocat ws-l:0.0.0.0:8084 tcp:[::1]:22
"#
);

#[derive(Debug, Clone)]
pub struct TcpListen(pub SocketAddr);
impl Specifier for TcpListen {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        multi(tcp_listen_peer(&self.0, p.left_to_right, p.program_options.announce_listens))
    }
    specifier_boilerplate!(noglobalstate multiconnect no_subspec );
}
specifier_class!(
    name = TcpListenClass,
    target = TcpListen,
    prefixes = ["tcp-listen:", "listen-tcp:", "tcp-l:", "l-tcp:"],
    arg_handling = parse,
    overlay = false,
    StreamOriented,
    MultiConnect,
    help = r#"
Listen TCP port on specified address.
    
Example: echo server

    websocat tcp-l:0.0.0.0:1441 mirror:
    
Example: redirect TCP to a websocket

    websocat tcp-l:0.0.0.0:8088 ws://echo.websocket.org
"#
);

#[derive(Debug, Clone)]
pub struct UdpConnect(pub SocketAddr);
impl Specifier for UdpConnect {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        once(udp_connect_peer(&self.0, &p.program_options))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = UdpConnectClass,
    target = UdpConnect,
    prefixes = ["udp:", "udp-connect:", "connect-udp:", "udp-c:", "c-udp:"],
    arg_handling = parse,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Send and receive packets to specified UDP socket, from random UDP port  
"#
);

#[derive(Debug, Clone)]
pub struct UdpListen(pub SocketAddr);
impl Specifier for UdpListen {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        once(udp_listen_peer(&self.0, &p.program_options))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = UdpListenClass,
    target = UdpListen,
    prefixes = ["udp-listen:", "listen-udp:", "udp-l:", "l-udp:"],
    arg_handling = parse,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Bind an UDP socket to specified host:port, receive packet
from any remote UDP socket, send replies to recently observed
remote UDP socket.

Note that it is not a multiconnect specifier like e.g. `tcp-listen`:
entire lifecycle of the UDP socket is the same connection.

File a feature request on Github if you want proper DNS-like request-reply UDP mode here.
"#
);

/*
struct RcReadProxy<R>(Rc<R>) where for<'a> &'a R : AsyncRead;

impl<R> AsyncRead for RcReadProxy<R> where for<'a> &'a R : AsyncRead{}
impl<R> Read for RcReadProxy<R> where for<'a> &'a R : AsyncRead {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        (&*self.0).read(buf)
    }
}

struct RcWriteProxy<W>(Rc<W>) where for<'a> &'a W : AsyncWrite;

impl<W> AsyncWrite for RcWriteProxy<W> where for<'a> &'a W : AsyncWrite {
    fn shutdown(&mut self) -> futures::Poll<(),std::io::Error> {
        (&*self.0).shutdown()
    }
}
impl<W> Write for RcWriteProxy<W> where for<'a> &'a W : AsyncWrite {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&*self.0).write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        (&*self.0).flush()
    }
}
*/

// based on https://github.com/tokio-rs/tokio-core/blob/master/examples/proxy.rs
#[derive(Clone)]
struct MyTcpStream(Rc<TcpStream>, bool);

impl Read for MyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for MyTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsyncRead for MyTcpStream {}

impl AsyncWrite for MyTcpStream {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.0.shutdown(std::net::Shutdown::Write)?;
        Ok(().into())
    }
}

impl Drop for MyTcpStream {
    fn drop(&mut self) {
        let i_am_read_part = self.1;
        if i_am_read_part {
            let _ = self.0.shutdown(std::net::Shutdown::Read);
        }
    }
}

pub fn tcp_connect_peer(addrs: &[SocketAddr]) -> BoxedNewPeerFuture {
    // Apply Happy Eyeballs in case of multiple proposed addresses.
    if addrs.len() > 1 {
        debug!("Setting up a race between multiple TCP client sockets. Who connects the first?");
    }
    use futures::stream::futures_unordered::FuturesUnordered;
    let mut fu = FuturesUnordered::new();
    for addr in addrs {
        let addr = addr.clone();
        fu.push(
            TcpStream::connect(&addr)
            .map(move |x| {
                info!("Connected to TCP {}", addr);
                let x = Rc::new(x);
                Peer::new(
                    MyTcpStream(x.clone(), true),
                    MyTcpStream(x.clone(), false),
                    None /* TODO */
                )
            })
            .map_err(box_up_err)
        );
    }
    // reverse Ok and Err variants so that `fold` would exit early on a successful connection, but accumulate errors.
    let p = fu.then(|x| {
        let reversed = match x {
            Ok(a) => Err(a),
            Err(a) => Ok(a),
        };
        futures::future::done(reversed)
    }).fold(None, |_accum, e|{
        log::info!("Failure during connecting TCP: {}", e);
        futures::future::ok(Some(e))
    }).then(|x| {
        match x {
            Ok(a) => Err(a),
            Err(a) => Ok(a),
        }
    }).map_err(|e : Option<_>| e.unwrap());
    /*let p = fu.into_future().and_then(|(x, _losers)| {
        let peer = x.unwrap();
        debug!("We have a winner. Disconnecting losers.");
        futures::future::ok(peer)       
    });*/
    //Box::new(p.map_err(|(e,_)|e)) as BoxedNewPeerFuture
    Box::new(p) as BoxedNewPeerFuture
}

pub fn tcp_listen_peer(addr: &SocketAddr, l2r: L2rUser, announce: bool) -> BoxedNewPeerStream {
    let bound = match TcpListener::bind(&addr) {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    debug!("Listening TCP socket");
    if announce {
        println!("LISTEN proto=tcp,ip={},port={}", addr.ip(), addr.port());
    }
    use tk_listen::ListenExt;
    Box::new(
        bound
            .incoming()
            .sleep_on_error(::std::time::Duration::from_millis(500))
            .map(move |x| {
                let addr = x.peer_addr().ok();
                info!("Incoming TCP connection from {:?}", addr);

                match l2r {
                    L2rUser::FillIn(ref y) => {
                        let mut z = y.borrow_mut();
                        z.client_addr = addr.map(|a| format!("{}", a));
                    }
                    L2rUser::ReadFrom(_) => {}
                }

                let x = Rc::new(x);
                Peer::new(
                    MyTcpStream(x.clone(), true),
                    MyTcpStream(x.clone(), false),
                    None, /* TODO */
                )
            })
            .map_err(|()| crate::simple_err2("unreachable error?")),
    ) as BoxedNewPeerStream
}

#[derive(Debug)]
enum UdpPeerState {
    ConnectMode,
    WaitingForAddress((Sender<()>, Receiver<()>)),
    HasAddress(SocketAddr),
}

struct UdpPeer {
    s: UdpSocket,
    state: Option<UdpPeerState>,
    oneshot_mode: bool,
}

#[derive(Clone)]
struct UdpPeerHandle(Rc<RefCell<UdpPeer>>);

fn get_zero_address(addr: &SocketAddr) -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let ip = match addr.ip() {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)),
    };
    SocketAddr::new(ip, 0)
}

fn apply_udp_options(s: &UdpSocket, opts:&Rc<Options>) -> IoResult<()> {
    if opts.udp_broadcast {
        s.set_broadcast(true)?;
    }
    let mut multicast_v4 = false;
    let mut multicast_v6 = false;

    let mut v4ai = opts.udp_join_multicast_iface_v4.iter();
    let mut v6ai = opts.udp_join_multicast_iface_v6.iter();

    let use_ai = opts.udp_join_multicast_iface_v4.len() + opts.udp_join_multicast_iface_v6.len() > 0;

    for multicast_address in opts.udp_join_multicast_addr.iter() {
        match multicast_address {
            std::net::IpAddr::V4(a) => {
                multicast_v4 = true;
                let interface_address = if use_ai {
                    *v4ai.next().unwrap()
                } else {
                    std::net::Ipv4Addr::UNSPECIFIED
                };
                s.join_multicast_v4(a, &interface_address)?;
            },
            std::net::IpAddr::V6(a) => {
                multicast_v6 = true;
                let interface_index = if use_ai {
                    *v6ai.next().unwrap()
                } else {
                    0
                };
                s.join_multicast_v6(a, interface_index)?;
            }
        }
    }

    if opts.udp_multicast_loop {
        if multicast_v4 {
            s.set_multicast_loop_v4(true)?;
        }
        if multicast_v6 {
            s.set_multicast_loop_v6(true)?;
        }
    }
    if let Some(ttl) = opts.udp_ttl {
        s.set_ttl(ttl)?;
        if multicast_v4 {
            s.set_multicast_ttl_v4(ttl)?;
        }
    }
    Ok(())
}

pub fn get_udp(addr: &SocketAddr, opts: &Rc<Options>) -> IoResult<UdpSocket> {
    let u = match addr {
        SocketAddr::V4(_) => net2::UdpBuilder::new_v4()?,
        SocketAddr::V6(_) => net2::UdpBuilder::new_v6()?,
    };
    if opts.udp_reuseaddr {
        u.reuse_address(true)?;
    }
    //u.only_v6(true);
    let u = u.bind(addr)?;
    UdpSocket::from_std(u, &tokio_reactor::Handle::default())
}

pub fn udp_connect_peer(addr: &SocketAddr, opts: &Rc<Options>) -> BoxedNewPeerFuture {
    let za = get_zero_address(addr);

    Box::new(futures::future::result(
        get_udp(&za, opts)
            .and_then(|x| {
                x.connect(addr)?;
                apply_udp_options(&x, opts)?;

                let h1 = UdpPeerHandle(Rc::new(RefCell::new(UdpPeer {
                    s: x,
                    state: Some(UdpPeerState::ConnectMode),
                    oneshot_mode: opts.udp_oneshot_mode,
                })));
                let h2 = h1.clone();
                Ok(Peer::new(h1, h2, None))
            })
            .map_err(box_up_err),
    )) as BoxedNewPeerFuture
}

pub fn udp_listen_peer(addr: &SocketAddr, opts: &Rc<Options>) -> BoxedNewPeerFuture {
    Box::new(futures::future::result(
        get_udp(addr, opts)
            .and_then(|x| {
                apply_udp_options(&x, opts)?;
                debug!("Ready for serving UDP");
                if opts.announce_listens {
                    println!("LISTEN proto=udp,ip={},port={}", addr.ip(), addr.port());
                }
                let h1 = UdpPeerHandle(Rc::new(RefCell::new(UdpPeer {
                    s: x,
                    state: Some(UdpPeerState::WaitingForAddress(channel())),
                    oneshot_mode: opts.udp_oneshot_mode,
                })));
                let h2 = h1.clone();
                Ok(Peer::new(h1, h2, None))
            })
            .map_err(box_up_err),
    )) as BoxedNewPeerFuture
}

impl Read for UdpPeerHandle {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut p = self.0.borrow_mut();
        match p.state.take().expect("Assertion failed 193912") {
            UdpPeerState::ConnectMode => {
                p.state = Some(UdpPeerState::ConnectMode);
                p.s.recv2(buf)
            }
            UdpPeerState::HasAddress(oldaddr) => {
                p.s.recv_from2(buf)
                    .map(|(ret, addr)| {
                        if addr != oldaddr {
                            warn!("New client for the same listening UDP socket");
                        }
                        p.state = Some(UdpPeerState::HasAddress(addr));
                        ret
                    })
                    .map_err(|e| {
                        p.state = Some(UdpPeerState::HasAddress(oldaddr));
                        e
                    })
            }
            UdpPeerState::WaitingForAddress((cmpl, pollster)) => match p.s.recv_from2(buf) {
                Ok((ret, addr)) => {
                    p.state = Some(UdpPeerState::HasAddress(addr));
                    let _ = cmpl.send(());
                    Ok(ret)
                }
                Err(e) => {
                    p.state = Some(UdpPeerState::WaitingForAddress((cmpl, pollster)));
                    Err(e)
                }
            },
        }
    }
}

impl Write for UdpPeerHandle {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let mut p = self.0.borrow_mut();
        match p.state.take().expect("Assertion failed 193913") {
            UdpPeerState::ConnectMode => {
                p.state = Some(UdpPeerState::ConnectMode);
                p.s.send2(buf)
            }
            UdpPeerState::HasAddress(a) => {
                if p.oneshot_mode {
                    p.state = Some(UdpPeerState::WaitingForAddress(channel()));
                } else {
                    p.state = Some(UdpPeerState::HasAddress(a));
                }
                p.s.send_to2(buf, &a)
            }
            UdpPeerState::WaitingForAddress((cmpl, mut pollster)) => {
                let _ = pollster.poll(); // register wakeup
                p.state = Some(UdpPeerState::WaitingForAddress((cmpl, pollster)));
                wouldblock()
            }
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsyncRead for UdpPeerHandle {}

impl AsyncWrite for UdpPeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(().into())
    }
}

/// Squirreled await from deprecated UdpSocket functions
trait UndeprecateNonpollSendRecv {
    fn recv2(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn recv_from2(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)>;
    fn send2(&mut self, buf: &[u8]) -> std::io::Result<usize>;
    fn send_to2(&mut self, buf: &[u8], target: &SocketAddr) -> std::io::Result<usize>;
}

impl UndeprecateNonpollSendRecv for UdpSocket {
    fn recv2(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.poll_recv(buf)? {
            futures::Async::Ready(n) => Ok(n),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }

    fn recv_from2(&mut self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        match self.poll_recv_from(buf)? {
            futures::Async::Ready(ret) => Ok(ret),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }

    fn send2(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.poll_send(buf)? {
            futures::Async::Ready(n) => Ok(n),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }

    fn send_to2(&mut self, buf: &[u8], target: &SocketAddr) -> std::io::Result<usize> {
        match self.poll_send_to(buf, target)? {
            futures::Async::Ready(n) => Ok(n),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }
}
