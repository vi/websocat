use futures;
use futures::future::Future;
use futures::stream::Stream;
use futures::unsync::oneshot::{channel, Receiver, Sender};
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::net::SocketAddr;
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};

use std::cell::RefCell;
use std::rc::Rc;

use tokio_core::net::{UdpSocket};
use tokio_tcp::{TcpStream, TcpListener};

use super::L2rUser;
use super::{box_up_err, peer_err_s, wouldblock, BoxedNewPeerFuture, BoxedNewPeerStream, Peer};
use super::{multi, once, ConstructParams, Options, PeerConstructor, Specifier};

#[derive(Debug, Clone)]
pub struct TcpConnect(pub SocketAddr);
impl Specifier for TcpConnect {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(tcp_connect_peer(&self.0))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec );
}
specifier_class!(
    name = TcpConnectClass,
    target = TcpConnect,
    prefixes = ["tcp:", "tcp-connect:", "connect-tcp:", "tcp-c:", "c-tcp:"],
    arg_handling = parse,
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
        multi(tcp_listen_peer(&self.0, p.left_to_right))
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
        once(udp_connect_peer(
            &p.tokio_handle,
            &self.0,
            &p.program_options,
        ))
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
        once(udp_listen_peer(
            &p.tokio_handle,
            &self.0,
            &p.program_options,
        ))
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
        try!(self.0.shutdown(std::net::Shutdown::Write));
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

pub fn tcp_connect_peer(addr: &SocketAddr) -> BoxedNewPeerFuture {
    Box::new(
        TcpStream::connect(&addr)
            .map(|x| {
                info!("Connected to TCP");
                let x = Rc::new(x);
                Peer::new(MyTcpStream(x.clone(), true), MyTcpStream(x.clone(), false))
            }).map_err(box_up_err),
    ) as BoxedNewPeerFuture
}

pub fn tcp_listen_peer(addr: &SocketAddr, l2r: L2rUser) -> BoxedNewPeerStream {
    let bound = match TcpListener::bind(&addr) {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    Box::new(
        bound
            .incoming()
            .map(move |x| {
                let addr = x.peer_addr().ok();
                info!("Incoming TCP connection from {:?}", addr);

                match l2r {
                    L2rUser::FillIn(ref y) => {
                        let mut z = y.borrow_mut();
                        z.client_addr = addr.map(|a|format!("{}", a));
                    }
                    L2rUser::ReadFrom(_) => {}
                }

                let x = Rc::new(x);
                Peer::new(MyTcpStream(x.clone(), true), MyTcpStream(x.clone(), false))
            }).map_err(box_up_err),
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

pub fn udp_connect_peer(
    handle: &Handle,
    addr: &SocketAddr,
    opts: &Rc<Options>,
) -> BoxedNewPeerFuture {
    let za = get_zero_address(addr);

    Box::new(futures::future::result(
        UdpSocket::bind(&za, handle)
            .and_then(|x| {
                x.connect(addr)?;

                let h1 = UdpPeerHandle(Rc::new(RefCell::new(UdpPeer {
                    s: x,
                    state: Some(UdpPeerState::ConnectMode),
                    oneshot_mode: opts.udp_oneshot_mode,
                })));
                let h2 = h1.clone();
                Ok(Peer::new(h1, h2))
            }).map_err(box_up_err),
    )) as BoxedNewPeerFuture
}

pub fn udp_listen_peer(
    handle: &Handle,
    addr: &SocketAddr,
    opts: &Rc<Options>,
) -> BoxedNewPeerFuture {
    Box::new(futures::future::result(
        UdpSocket::bind(addr, handle)
            .and_then(|x| {
                let h1 = UdpPeerHandle(Rc::new(RefCell::new(UdpPeer {
                    s: x,
                    state: Some(UdpPeerState::WaitingForAddress(channel())),
                    oneshot_mode: opts.udp_oneshot_mode,
                })));
                let h2 = h1.clone();
                Ok(Peer::new(h1, h2))
            }).map_err(box_up_err),
    )) as BoxedNewPeerFuture
}

impl Read for UdpPeerHandle {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let mut p = self.0.borrow_mut();
        match p.state.take().expect("Assertion failed 193912") {
            UdpPeerState::ConnectMode => {
                p.state = Some(UdpPeerState::ConnectMode);
                p.s.recv(buf)
            }
            UdpPeerState::HasAddress(oldaddr) => {
                p.s.recv_from(buf)
                    .map(|(ret, addr)| {
                        warn!("New client for the same listening UDP socket");
                        p.state = Some(UdpPeerState::HasAddress(addr));
                        ret
                    }).map_err(|e| {
                        p.state = Some(UdpPeerState::HasAddress(oldaddr));
                        e
                    })
            }
            UdpPeerState::WaitingForAddress((cmpl, pollster)) => match p.s.recv_from(buf) {
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
                p.s.send(buf)
            }
            UdpPeerState::HasAddress(a) => {
                if p.oneshot_mode {
                    p.state = Some(UdpPeerState::WaitingForAddress(channel()));
                } else {
                    p.state = Some(UdpPeerState::HasAddress(a));
                }
                p.s.send_to(buf, &a)
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
