use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpSocket, TcpStream};

pub struct TcpBindOptions {
    pub bind_before_connecting: Option<SocketAddr>,
    pub reuseaddr: Option<bool>,
    pub reuseport: bool,
    pub bind_device: Option<String>,
    pub listen_backlog: u32,
    pub freebind: bool,
    pub transparent: bool,
}

pub struct TcpStreamOptions {
    pub tclass_v6: Option<u32>,
    pub tos_v4: Option<u32>,
    /// Or hops limit for IPv6
    pub ttl: Option<u32>,
    pub tcp_congestion: Option<String>,
    pub cpu_affinity: Option<usize>,
    pub user_timeout_s: Option<u32>,
    pub linger_s: Option<u32>,
    pub out_of_band_inline: bool,
    pub priority: Option<u32>,
    pub recv_buffer_size: Option<usize>,
    pub send_buffer_size: Option<usize>,
    pub only_v6: Option<bool>,
    pub nodelay: Option<bool>,
    pub mss: Option<u32>,
    pub mark: Option<u32>,
    pub thin_linear_timeouts: Option<bool>,
    pub notsent_lowat: Option<u32>,

    pub keepalive: Option<bool>,
    pub keepalive_retries: Option<u32>,
    pub keepalive_interval_s: Option<u32>,
    pub keepalive_idletime_s: Option<u32>,
}

macro_rules! cfg_gated_block_or_err {
    ($feature:literal, #[cfg($($c:tt)*)], $b:block$ (,)?) => {
        'a: {
            #[cfg($($c)*)] {
                $b;
                break 'a;
            }
            #[allow(unreachable_code)]
            return Err(std::io::Error::new(std::io::ErrorKind::Other, concat!("Not supported on this platform: `",$feature,"`")))
        }
    };
}

#[macro_export]
macro_rules! copy_common_tcp_bind_options {
    ($target:ident, $source:ident) => {
        $target.reuseaddr = $source.reuseaddr;
        $target.reuseport = $source.reuseport;
        $target.bind_device = $source.bind_device;
        $target.freebind = $source.freebind;
        $target.transparent = $source.transparent;
    };
}

struct SocketWrapper(Option<socket2::Socket>);

impl Drop for SocketWrapper {
    fn drop(&mut self) {
        #[cfg(unix)] {
            use std::os::fd::IntoRawFd;
            if let Some(s) = self.0.take() {
                let _ = s.into_raw_fd();
            }
        }
    }
}

#[cfg(unix)]
impl<T> From<&T> for SocketWrapper where T : std::os::fd::AsRawFd {
    fn from(s: &T) -> Self {
        use std::os::fd::FromRawFd;
        SocketWrapper(Some(
            // Safety: resulting `socket2::Socket` is expected to only be used from this module to set some options and
            // is quickly forgotten (by `Drop` implementation above), so it feels it should be more or less OK.
            unsafe {
                socket2::Socket::from_raw_fd(s.as_raw_fd())
            }
        ))
    }
}

impl std::ops::Deref for SocketWrapper {
    type Target = socket2::Socket;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl TcpBindOptions {
    pub fn new() -> TcpBindOptions {
        Self {
            bind_before_connecting: None,
            reuseaddr: None,
            reuseport: false,
            bind_device: None,
            listen_backlog: 1024,

            freebind: false,
            transparent: false,
        }
    }

    fn gs4a(addr: SocketAddr) -> std::io::Result<TcpSocket> {
        if addr.is_ipv4() {
            TcpSocket::new_v4()
        } else if addr.is_ipv6() {
            TcpSocket::new_v6()
        } else {
            panic!("Non IPv4 or IPv6 address is specified for a TCP socket");
        }
    }

    fn setopts(&self, s: &TcpSocket, v6: bool, pending_listen: bool) -> std::io::Result<()> {
        if let Some(v) = self.reuseaddr {
            s.set_reuseaddr(v)?;
        } else {
            if pending_listen {
                #[cfg(not(windows))]
                s.set_reuseaddr(true)?;
            }
        }
        if self.reuseport {
            cfg_gated_block_or_err!(
                "reuseport",
                #[cfg(all(
                    unix,
                    not(target_os = "solaris"),
                    not(target_os = "illumos"),
                    not(target_os = "cygwin"),
                ))],
                {
                    s.set_reuseport(true)?;
                },
            );
        }
        if self.transparent {
            cfg_gated_block_or_err!(
                "transparent",
                #[cfg(target_os = "linux")],
                {
                    let ss : SocketWrapper = s.into();
                    ss.set_ip_transparent(true)?;
                },
            );
        }
        if self.freebind {
            if v6 {
                cfg_gated_block_or_err!(
                    "freebind",
                    #[cfg(any(target_os = "android", target_os = "linux"))],
                    {
                        let ss : SocketWrapper = s.into();
                        ss.set_freebind_ipv6(true)?;
                    },
                );
            } else {
                cfg_gated_block_or_err!(
                    "freebind",
                    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                    {
                        let ss : SocketWrapper = s.into();
                        ss.set_freebind(true)?;
                    },
                );
            }
        }
        if let Some(ref v) = self.bind_device {
            cfg_gated_block_or_err!(
                "bind_device",
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                {
                    s.bind_device(Some(v[..].as_bytes()))?;
                },
            );
        }
        Ok(())
    }

    pub async fn connect(&self, addr: SocketAddr) -> std::io::Result<TcpStream> {
        let s = Self::gs4a(addr)?;
        self.setopts(&s, addr.is_ipv6(), false)?;
        if let Some(bbc) = self.bind_before_connecting {
            s.bind(bbc)?;
        }
        s.connect(addr).await
        //TcpStream::connect(addr).await
    }

    pub async fn bind(&self, addr: SocketAddr) -> std::io::Result<TcpListener> {
        let s = Self::gs4a(addr)?;
        self.setopts(&s, addr.is_ipv6(), true)?;
        s.bind(addr)?;
        s.listen(self.listen_backlog)
        //TcpListener::bind(addr).await
    }
}

impl TcpStreamOptions {
    pub fn new() -> TcpStreamOptions {
        Self {
            tclass_v6: None,
            tos_v4: None,
            ttl: None,
            tcp_congestion: None,
            cpu_affinity: None,
            user_timeout_s: None,
            linger_s: None,
            out_of_band_inline: false,
            priority: None,
            recv_buffer_size: None,
            send_buffer_size: None,
            only_v6: None,
            nodelay: None,
            mss: None,
            mark: None,
            thin_linear_timeouts: None,
            notsent_lowat: None,
            keepalive: None,
            keepalive_retries: None,
            keepalive_interval_s: None,
            keepalive_idletime_s: None,
        }
    }
}
