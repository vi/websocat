extern crate tokio_reactor;

use super::{
    futures, libc, multi, once, peer_err_s, simple_err, BoxedNewPeerFuture, BoxedNewPeerStream,
    ConstructParams, MyUnixStream, Options, Peer, PeerConstructor, Specifier, UnixListener,
    UnixStream,
};
use futures::Stream;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct SeqpacketConnect(pub PathBuf);
impl Specifier for SeqpacketConnect {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        once(seqpacket_connect_peer(&self.0))
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = SeqpacketConnectClass,
    target = SeqpacketConnect,
    prefixes = [
        "seqpacket:",
        "seqpacket-connect:",
        "connect-seqpacket:",
        "seqpacket-c:",
        "c-seqpacket:"
    ],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    SingleConnect,
    help = r#"
Connect to AF_UNIX SOCK_SEQPACKET socket. Argument is a filesystem path. [A]

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long paths are silently truncated.

Example: forward connections from websockets to a UNIX seqpacket abstract socket

    websocat ws-l:127.0.0.1:1234 seqpacket:@test
"#
);

#[derive(Debug, Clone)]
pub struct SeqpacketListen(pub PathBuf);
impl Specifier for SeqpacketListen {
    fn construct(&self, p: ConstructParams) -> PeerConstructor {
        multi(seqpacket_listen_peer(&self.0, &p.program_options))
    }
    specifier_boilerplate!(noglobalstate multiconnect no_subspec);
}
specifier_class!(
    name = SeqpacketListenClass,
    target = SeqpacketListen,
    prefixes = [
        "seqpacket-listen:",
        "listen-seqpacket:",
        "seqpacket-l:",
        "l-seqpacket:"
    ],
    arg_handling = into,
    overlay = false,
    MessageOriented,
    MultiConnect,
    help = r#"
Listen for connections on a specified AF_UNIX SOCK_SEQPACKET socket [A]

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long (>=108 bytes) paths are silently truncated.

Example: forward connections from a UNIX seqpacket socket to a WebSocket

    websocat --unlink seqpacket-l:the_socket ws://127.0.0.1:8089
"#
);

pub fn seqpacket_connect_peer(addr: &Path) -> BoxedNewPeerFuture {
    fn getfd(addr: &Path) -> Option<i32> {
        use self::libc::{
            c_char, close, connect, sa_family_t, sockaddr_un, socket, socklen_t, AF_UNIX,
            SOCK_SEQPACKET,
        };
        use std::mem::size_of;
        use std::os::unix::ffi::OsStrExt;
        unsafe {
            let s = socket(AF_UNIX, SOCK_SEQPACKET, 0);
            if s == -1 {
                return None;
            }
            {
                let mut sa = sockaddr_un {
                    sun_family: AF_UNIX as sa_family_t,
                    sun_path: [0; 108],
                };
                let bp: &[c_char] =
                    &*(addr.as_os_str().as_bytes() as *const [u8] as *const [c_char]);
                let l = 108.min(bp.len());
                sa.sun_path[..l].copy_from_slice(&bp[..l]);
                if sa.sun_path[0] == b'@' as c_char {
                    sa.sun_path[0] = b'\x00' as c_char;
                }
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
    fn getpeer(addr: &Path) -> Result<Peer, Box<dyn (::std::error::Error)>> {
        if let Some(fd) = getfd(addr) {
            let s: ::std::os::unix::net::UnixStream =
                unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) };
            let ss = UnixStream::from_std(s, &tokio_reactor::Handle::default())?;
            let x = Rc::new(ss);
            Ok(Peer::new(
                MyUnixStream(x.clone(), true),
                MyUnixStream(x.clone(), false),
                None /* TODO*/ ,
            ))
        } else {
            Err("Failed to get or connect socket")?
        }
    }
    Box::new(futures::future::result(getpeer(addr))) as BoxedNewPeerFuture
}

pub fn seqpacket_listen_peer(addr: &Path, opts: &Rc<Options>) -> BoxedNewPeerStream {
    fn getfd(addr: &Path, opts: &Rc<Options>) -> Option<i32> {
        use self::libc::{
            bind, c_char, close, listen, sa_family_t, sockaddr_un, socket, socklen_t, unlink,
            AF_UNIX, SOCK_SEQPACKET,
        };
        use std::mem::size_of;
        use std::os::unix::ffi::OsStrExt;
        unsafe {
            let s = socket(AF_UNIX, SOCK_SEQPACKET, 0);
            if s == -1 {
                return None;
            }
            {
                let mut sa = sockaddr_un {
                    sun_family: AF_UNIX as sa_family_t,
                    sun_path: [0; 108],
                };
                let bp: &[c_char] =
                    &*(addr.as_os_str().as_bytes() as *const [u8] as *const [c_char]);

                let l = 108.min(bp.len());
                sa.sun_path[..l].copy_from_slice(&bp[..l]);
                if sa.sun_path[0] == b'@' as c_char {
                    sa.sun_path[0] = b'\x00' as c_char;
                } else if opts.unlink_unix_socket {
                    sa.sun_path[107] = 0;
                    unlink(&sa.sun_path as *const c_char);
                }
                let sa_len = l + size_of::<sa_family_t>();
                let sa_ = &sa as *const self::libc::sockaddr_un as *const self::libc::sockaddr;
                let ret = bind(s, sa_, sa_len as socklen_t);
                if ret == -1 {
                    close(s);
                    return None;
                }
            }
            {
                let ret = listen(s, 50);
                if ret == -1 {
                    close(s);
                    return None;
                }
            }
            if opts.announce_listens {
                // too lazy to actually handle '"@'  vs '@"' here - is seqpacket even used by somebody around?
                let s = format!("LISTEN proto=unix_seqpacket,path={:?}", addr);
                if s.contains("path=\"@") {
                    warn!("that particular LISTEN line format should be changed in future Websocat version");
                }
                println!("{}", s);
            }
            Some(s)
        }
    }
    let fd = match getfd(addr, opts) {
        Some(x) => x,
        None => return peer_err_s(simple_err("Failed to get or bind socket".into())),
    };
    let l1: ::std::os::unix::net::UnixListener =
        unsafe { ::std::os::unix::io::FromRawFd::from_raw_fd(fd) };
    let bound = match UnixListener::from_std(l1, &tokio_reactor::Handle::default()) {
        Ok(x) => x,
        Err(e) => return peer_err_s(e),
    };
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
                    None /* TODO*/ ,
                )
            })
            .map_err(|()| crate::simple_err2("unreachable error?")),
    ) as BoxedNewPeerStream
}
