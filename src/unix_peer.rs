extern crate tokio_reactor;
extern crate tokio_uds;

extern crate libc;

use futures;
use futures::stream::Stream;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::cell::RefCell;
use std::rc::Rc;

use std::path::{Path, PathBuf};

use self::tokio_uds::{UnixDatagram, UnixListener, UnixStream};

//#[cfg_attr(feature="cargo-clippy",allow(unused_imports))]
#[allow(unused_imports)]
use super::simple_err;
use super::{box_up_err, peer_err_s, util::peer_err_sb, BoxedNewPeerFuture, BoxedNewPeerStream, Peer};
use super::{multi, once, ConstructParams, Options, PeerConstructor, Specifier};

#[derive(Debug, Clone)]
pub struct UnixConnect(pub PathBuf);
impl Specifier for UnixConnect {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(unix_connect_peer(&self.0))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = UnixConnectClass,
    target = UnixConnect,
    prefixes = [
        "unix:",
        "unix-connect:",
        "connect-unix:",
        "unix-c:",
        "c-unix:"
    ],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Connect to UNIX socket. Argument is filesystem path. [A]

Example: forward connections from websockets to a UNIX stream socket

    websocat ws-l:127.0.0.1:8088 unix:the_socket
"#
);

#[derive(Debug, Clone)]
pub struct UnixListen(pub PathBuf);
impl Specifier for UnixListen {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        multi(unix_listen_peer(&self.0, &p.program_options))
    }
    specifier_boilerplate!(noglobalstate multiconnect no_subspec);
}
specifier_class!(
    name = UnixListenClass,
    target = UnixListen,
    prefixes = ["unix-listen:", "listen-unix:", "unix-l:", "l-unix:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    MultiConnect,
    help = r#"
Listen for connections on a specified UNIX socket [A]

Example: forward connections from a UNIX socket to a WebSocket

    websocat --unlink unix-l:the_socket ws://127.0.0.1:8089

Example: Accept forwarded WebSocket connections from Nginx

    umask 0000
    websocat --unlink -b -E ws-u:unix-l:/tmp/wstest tcp:[::]:22

Nginx config:

    location /ws {
        proxy_read_timeout 7d;
        proxy_send_timeout 7d;
        #proxy_pass http://localhost:3012;
        proxy_pass http://unix:/tmp/wstest;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection \"upgrade\";
    }

This configuration allows to make Nginx responsible for
SSL and also it can choose which connections to forward
to websocat based on URLs.

Obviously, Nginx can also redirect to TCP-listening
websocat just as well - UNIX sockets are not a requirement for this feature.

See `moreexamples.md` for SystemD usage (untested).

TODO: --chmod option?
"#
);

#[derive(Debug, Clone)]
pub struct UnixDgram(pub PathBuf, pub PathBuf);
impl Specifier for UnixDgram {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        once(dgram_peer(&self.0, &self.1, &p.program_options))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = UnixDgramClass,
    target = UnixDgram,
    prefixes = ["unix-dgram:"],
    arg_handling = {
        fn construct(self: &UnixDgramClass, just_arg: &str) -> super::Result<Rc<dyn Specifier>> {
            let splits: Vec<&str> = just_arg.split(':').collect();
            if splits.len() != 2 {
                Err("Expected two colon-separated paths")?;
            }
            Ok(Rc::new(UnixDgram(splits[0].into(), splits[1].into())))
        }
        fn construct_overlay(
            self: &UnixDgramClass,
            _inner: Rc<dyn Specifier>,
        ) -> super::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
    },
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Send packets to one path, receive from the other. [A]
A socket for sending must be already opened.

I don't know if this mode has any use, it is here just for completeness.

Example:

    socat unix-recv:./sender -&
    websocat - unix-dgram:./receiver:./sender
"#
);

fn to_abstract(x: &str) -> PathBuf {
    format!("\x00{}", x).into()
}

#[derive(Debug, Clone)]
pub struct AbstractConnect(pub String);
impl Specifier for AbstractConnect {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(unix_connect_peer(&to_abstract(&self.0)))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = AbstractConnectClass,
    target = AbstractConnect,
    prefixes = [
        "abstract:",
        "abstract-connect:",
        "connect-abstract:",
        "abstract-c:",
        "c-abstract:"
    ],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Connect to UNIX abstract-namespaced socket. Argument is some string used as address. [A]

Too long addresses may be silently chopped off.

Example: forward connections from websockets to an abstract stream socket

    websocat ws-l:127.0.0.1:8088 abstract:the_socket

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.
"#
);

#[derive(Debug, Clone)]
pub struct AbstractListen(pub String);
impl Specifier for AbstractListen {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        multi(unix_listen_peer(
            &to_abstract(&self.0),
            &cp.program_options,
        ))
    }
    specifier_boilerplate!(noglobalstate multiconnect no_subspec);
}
specifier_class!(
    name = AbstractListenClass,
    target = AbstractListen,
    prefixes = [
        "abstract-listen:",
        "listen-abstract:",
        "abstract-l:",
        "l-abstract:"
    ],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    MultiConnect,
    help = r#"
Listen for connections on a specified abstract UNIX socket [A]

Example: forward connections from an abstract UNIX socket to a WebSocket

    websocat abstract-l:the_socket ws://127.0.0.1:8089

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.
"#
);

#[derive(Debug, Clone)]
pub struct AbstractDgram(pub String, pub String);
impl Specifier for AbstractDgram {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        #[cfg(not(all(target_os = "linux", feature = "workaround1")))]
        {
            once(dgram_peer(
                &to_abstract(&self.0),
                &to_abstract(&self.1),
                &p.program_options,
            ))
        }
        #[cfg(all(target_os = "linux", feature = "workaround1"))]
        {
            once(dgram_peer_workaround(
                &to_abstract(&self.0),
                &to_abstract(&self.1),
                &p.program_options,
            ))
        }
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = AbstractDgramClass,
    target = AbstractDgram,
    prefixes = ["abstract-dgram:"],
    arg_handling = {
        fn construct(self: &AbstractDgramClass, just_arg: &str) -> super::Result<Rc<dyn Specifier>> {
            let splits: Vec<&str> = just_arg.split(':').collect();
            if splits.len() != 2 {
                Err("Expected two colon-separated addresses")?;
            }
            Ok(Rc::new(UnixDgram(splits[0].into(), splits[1].into())))
        }
        fn construct_overlay(
            self: &AbstractDgramClass,
            _inner: Rc<dyn Specifier>,
        ) -> super::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
    },
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Send packets to one address, receive from the other. [A]
A socket for sending must be already opened.

I don't know if this mode has any use, it is here just for completeness.

Example (untested):

    websocat - abstract-dgram:receiver_addr:sender_addr

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them. In particular, this mode
may fail to work without `workaround1` Cargo feature.
"#
);

#[cfg(all(target_os = "linux", feature = "seqpacket"))]
#[path = "unix_seqpacket_peer.rs"]
pub mod unix_seqpacket_peer;

// based on https://github.com/tokio-rs/tokio-core/blob/master/examples/proxy.rs
#[derive(Clone)]
struct MyUnixStream(Rc<UnixStream>, bool);

impl Read for MyUnixStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for MyUnixStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsyncRead for MyUnixStream {}

impl AsyncWrite for MyUnixStream {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.0.shutdown(std::net::Shutdown::Write)?;
        Ok(().into())
    }
}

impl Drop for MyUnixStream {
    fn drop(&mut self) {
        let i_am_read_part = self.1;
        if i_am_read_part {
            let _ = self.0.shutdown(std::net::Shutdown::Read);
        }
    }
}

pub fn unix_connect_peer(addr: &Path) -> BoxedNewPeerFuture {
    use futures::Future;
    Box::new(
        UnixStream::connect(&addr)
            .map(|x| {
                info!("Connected to a unix socket");
                let x = Rc::new(x);
                Peer::new(
                    MyUnixStream(x.clone(), true),
                    MyUnixStream(x.clone(), false),
                    None /* TODO */,
                )
            })
            .map_err(box_up_err),
    ) as BoxedNewPeerFuture
}

pub fn unix_listen_peer(addr: &Path, opts: &Rc<Options>) -> BoxedNewPeerStream {
    let bound = if opts.unix_socket_accept_from_fd {
        // Special mode for SystemD (untested yet)
        let fdnum: libc::c_int = match addr.to_str().map(|x|x.parse()) {
            Some(Ok(x)) => x,
            _ => {
                let e: Box<dyn std::error::Error> = From::from("Specify numeric argument instead of path in --accept-from-fd mode");
                return peer_err_sb(e);
            }
        };
        use std::os::unix::io::FromRawFd;
        let l = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fdnum) } ;
        let _ = l.set_nonblocking(true);
        let bound =
        UnixListener::from_std(l, &tokio_reactor::Handle::default());
        bound
    } else {
        if opts.unlink_unix_socket {
            let _ = ::std::fs::remove_file(addr);
        };
        let bound = UnixListener::bind(&addr);
        if opts.announce_listens {
            let poss = addr.as_os_str();
            use std::os::unix::ffi::OsStrExt;
            if !poss.is_empty() && poss.as_bytes()[0] == b'\0' {
                println!("LISTEN proto=abstract,path_hex={}", hex::encode(&poss.as_bytes()[1..]));
            } else {
                println!("LISTEN proto=unix,path={:?}", addr);
            }
        }
        bound
    };
    let bound = match bound {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
    debug!("UNIX listening socket should be ready");
    // TODO: chmod
    use tk_listen::ListenExt;
    Box::new(
        bound
            .incoming()
            .sleep_on_error(::std::time::Duration::from_millis(500))
            .map(|x| {
                info!("Incoming unix socket connection");
                let x = Rc::new(x);
                Peer::new(
                    MyUnixStream(x.clone(), true),
                    MyUnixStream(x.clone(), false),
                    None /* TODO */,
                )
            })
            .map_err(|()| crate::simple_err2("unreachable error?")),
    ) as BoxedNewPeerStream
}

struct DgramPeer {
    s: UnixDatagram,
    #[allow(unused)]
    oneshot_mode: bool, // TODO
}

#[derive(Clone)]
struct DgramPeerHandle(Rc<RefCell<DgramPeer>>);

pub fn dgram_peer(bindaddr: &Path, connectaddr: &Path, opts: &Rc<Options>) -> BoxedNewPeerFuture {
    Box::new(futures::future::result(
        UnixDatagram::bind(bindaddr)
            .and_then(|x| {
                x.connect(connectaddr)?;

                let h1 = DgramPeerHandle(Rc::new(RefCell::new(DgramPeer {
                    s: x,
                    oneshot_mode: opts.udp_oneshot_mode,
                })));
                let h2 = h1.clone();
                Ok(Peer::new(h1, h2, None))
            })
            .map_err(box_up_err),
    )) as BoxedNewPeerFuture
}

#[cfg(all(target_os = "linux", feature = "workaround1"))]
pub fn dgram_peer_workaround(
    bindaddr: &Path,
    connectaddr: &Path,
    opts: &Rc<Options>,
) -> BoxedNewPeerFuture {
    info!("Workaround method for getting abstract datagram socket");
    fn getfd(bindaddr: &Path, connectaddr: &Path) -> Option<i32> {
        use self::libc::{
            bind, c_char, close, connect, sa_family_t, sockaddr_un, socket, socklen_t, AF_UNIX,
            SOCK_DGRAM,
        };
        use std::mem::size_of;
        use std::os::unix::ffi::OsStrExt;
        unsafe {
            let s = socket(AF_UNIX, SOCK_DGRAM, 0);
            if s == -1 {
                return None;
            }
            {
                let mut sa = sockaddr_un {
                    sun_family: AF_UNIX as sa_family_t,
                    sun_path: [0; 108],
                };
                let bp: &[c_char] =
                    &*(bindaddr.as_os_str().as_bytes() as *const [u8] as *const [c_char]);
                let l = 108.min(bp.len());
                sa.sun_path[..l].copy_from_slice(&bp[..l]);
                let sa_len = l + size_of::<sa_family_t>();
                let sa_ = &sa as *const self::libc::sockaddr_un as *const self::libc::sockaddr;
                let ret = bind(s, sa_, sa_len as socklen_t);
                if ret == -1 {
                    close(s);
                    return None;
                }
            }
            {
                let mut sa = sockaddr_un {
                    sun_family: AF_UNIX as sa_family_t,
                    sun_path: [0; 108],
                };
                let bp: &[c_char] =
                    &*(connectaddr.as_os_str().as_bytes() as *const [u8] as *const [c_char]);
                let l = 108.min(bp.len());
                sa.sun_path[..l].copy_from_slice(&bp[..l]);
                let sa_len = l + size_of::<sa_family_t>();
                let sa_ = &sa as *const self::libc::sockaddr_un as *const self::libc::sockaddr;
                let ret = connect(s, sa_, sa_len as socklen_t);
                if ret == -1 {
                    close(s);
                    return None;
                }
            }
            Some(s)
        }
    }
    fn getpeer(
        bindaddr: &Path,
        connectaddr: &Path,
        opts: &Rc<Options>,
    ) -> Result<Peer, Box<dyn (::std::error::Error)>> {
        if let Some(fd) = getfd(bindaddr, connectaddr) {
            let s: ::std::os::unix::net::UnixDatagram =
                unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) };
            let ss = UnixDatagram::from_std(s, &tokio_reactor::Handle::default())?;
            let h1 = DgramPeerHandle(Rc::new(RefCell::new(DgramPeer {
                s: ss,
                oneshot_mode: opts.udp_oneshot_mode,
            })));
            let h2 = h1.clone();
            Ok(Peer::new(h1, h2, None))
        } else {
            Err("Failed to get, bind or connect socket")?
        }
    }
    Box::new(futures::future::result({
        getpeer(bindaddr, connectaddr, opts)
    })) as BoxedNewPeerFuture
}

impl Read for DgramPeerHandle {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let p = self.0.borrow_mut();
        p.s.recv(buf)
    }
}

impl Write for DgramPeerHandle {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let p = self.0.borrow_mut();
        p.s.send(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl AsyncRead for DgramPeerHandle {}

impl AsyncWrite for DgramPeerHandle {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(().into())
    }
}

trait HacksForMigratingFromTokioCore {
    fn recv(&self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn send(&self, buf: &[u8]) -> std::io::Result<usize>;
}

impl HacksForMigratingFromTokioCore for UnixDatagram {
    fn recv(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.poll_recv(buf)? {
            futures::Async::Ready(n) => Ok(n),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }

    fn send(&self, buf: &[u8]) -> std::io::Result<usize> {
        match self.poll_send(buf)? {
            futures::Async::Ready(n) => Ok(n),
            futures::Async::NotReady => Err(std::io::ErrorKind::WouldBlock.into()),
        }
    }
}
