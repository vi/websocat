use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tracing::debug;

pub struct TcpBindOptions {
    pub(crate) bind_before_connecting: Option<SocketAddr>,
    pub(crate) reuseaddr: Option<bool>,
    pub(crate) reuseport: bool,
    pub(crate) bind_device: Option<String>,
    pub(crate) listen_backlog: u32,
    pub(crate) freebind: bool,
    pub(crate) transparent: bool,
    pub(crate) only_v6: Option<bool>,
}

pub struct TcpStreamOptions {
    pub(crate) tclass_v6: Option<u32>,
    pub(crate) tos_v4: Option<u32>,
    /// Or hops limit for IPv6
    pub(crate) ttl: Option<u32>,
    pub(crate) linger_s: Option<u32>,
    pub(crate) out_of_band_inline: bool,
    pub(crate) nodelay: Option<bool>,

    pub(crate) tcp_congestion: Option<String>,
    pub(crate) cpu_affinity: Option<usize>,
    pub(crate) user_timeout_s: Option<u32>,
    pub(crate) priority: Option<u32>,
    pub(crate) recv_buffer_size: Option<usize>,
    pub(crate) send_buffer_size: Option<usize>,
    pub(crate) mss: Option<u32>,
    pub(crate) mark: Option<u32>,
    pub(crate) thin_linear_timeouts: Option<bool>,
    pub(crate) notsent_lowat: Option<u32>,

    pub(crate) keepalive: Option<bool>,
    pub(crate) keepalive_retries: Option<u32>,
    pub(crate) keepalive_interval_s: Option<u32>,
    pub(crate) keepalive_idletime_s: Option<u32>,
}

macro_rules! cfg_gated_block_or_err {
    ($feature:literal, #[cfg($($c:tt)*)], $b:block$ (,)?) => {
        #[allow(unused_labels)]
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
        $target.only_v6 = $source.only_v6;
    };
}

#[macro_export]
macro_rules! copy_common_tcp_stream_options {
    ($target:ident, $source:ident) => {
        $target.tclass_v6 = $source.tclass_v6;
        $target.tos_v4 = $source.tos_v4;
        $target.ttl = $source.ttl;
        $target.linger_s = $source.linger_s;
        $target.out_of_band_inline = $source.out_of_band_inline;
        $target.nodelay = $source.nodelay;
        $target.tcp_congestion = $source.tcp_congestion;
        $target.cpu_affinity = $source.cpu_affinity;
        $target.user_timeout_s = $source.user_timeout_s;
        $target.priority = $source.priority;
        $target.recv_buffer_size = $source.recv_buffer_size;
        $target.send_buffer_size = $source.send_buffer_size;
        $target.mss = $source.mss;
        $target.mark = $source.mark;
        $target.thin_linear_timeouts = $source.thin_linear_timeouts;
        $target.notsent_lowat = $source.notsent_lowat;
        $target.keepalive = $source.keepalive;
        $target.keepalive_retries = $source.keepalive_retries;
        $target.keepalive_interval_s = $source.keepalive_interval_s;
        $target.keepalive_idletime_s = $source.keepalive_idletime_s;
    };
}

/// socket2::Socket wrapper that forgets the socket on drop instead of closing it
struct SocketWrapper(Option<socket2::Socket>);

#[cfg(unix)]
impl Drop for SocketWrapper {
    fn drop(&mut self) {
        use std::os::fd::IntoRawFd;
        if let Some(s) = self.0.take() {
            let _ = s.into_raw_fd();
        }
    }
}

#[cfg(unix)]
impl<T> From<&T> for SocketWrapper
where
    T: std::os::fd::AsRawFd,
{
    fn from(s: &T) -> Self {
        use std::os::fd::FromRawFd;
        SocketWrapper(Some(
            // Safety: resulting `socket2::Socket` is expected to only be used from this module to set some options and
            // is quickly forgotten (by `Drop` implementation above), so it feels it should be more or less OK.
            unsafe { socket2::Socket::from_raw_fd(s.as_raw_fd()) },
        ))
    }
}

#[cfg(windows)]
impl Drop for SocketWrapper {
    fn drop(&mut self) {
        use std::os::windows::io::IntoRawSocket;
        if let Some(s) = self.0.take() {
            let _ = s.into_raw_socket();
        }
    }
}

#[cfg(windows)]
impl<T> From<&T> for SocketWrapper
where
    T: std::os::windows::io::AsRawSocket,
{
    fn from(s: &T) -> Self {
        use std::os::windows::io::FromRawSocket;
        SocketWrapper(Some(
            // Safety: resulting `socket2::Socket` is expected to only be used from this module to set some options and
            // is quickly forgotten (by `Drop` implementation above), so it feels it should be more or less OK.
            unsafe { socket2::Socket::from_raw_socket(s.as_raw_socket()) },
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

            only_v6: None,
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
            debug!("Setting SO_REUSEADDR");
            s.set_reuseaddr(v)?;
        } else if pending_listen {
            #[cfg(not(windows))]
            s.set_reuseaddr(true)?;
        }
        if self.reuseport {
            debug!("Setting SO_REUSEPORT");
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
            debug!("Setting IP_TRANSPARENT");
            cfg_gated_block_or_err!(
                "transparent",
                #[cfg(target_os = "linux")],
                {
                    let ss : SocketWrapper = s.into();
                    ss.set_ip_transparent_v4(true)?;
                },
            );
        }
        if self.freebind {
            if v6 {
                debug!("Setting IPV6_FREEBIND");
                cfg_gated_block_or_err!(
                    "freebind",
                    #[cfg(any(target_os = "android", target_os = "linux"))],
                    {
                        let ss : SocketWrapper = s.into();
                        ss.set_freebind_v6(true)?;
                    },
                );
            } else {
                debug!("Setting IP_FREEBIND");
                cfg_gated_block_or_err!(
                    "freebind",
                    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                    {
                        let ss : SocketWrapper = s.into();
                        ss.set_freebind_v4(true)?;
                    },
                );
            }
        }
        if let Some(ref v) = self.bind_device {
            debug!("Setting SO_BINDTODEVICE");
            cfg_gated_block_or_err!(
                "bind_device",
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                {
                    s.bind_device(Some(v.as_bytes()))?;
                },
            );
        }

        if v6 {
            #[cfg(any(windows, unix))]
            if let Some(v) = self.only_v6 {
                let ss: SocketWrapper = s.into();
                ss.set_only_v6(v)?;
            }
        }
        Ok(())
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
        stream_opts: &TcpStreamOptions,
    ) -> std::io::Result<TcpStream> {
        let s = Self::gs4a(addr)?;
        self.setopts(&s, addr.is_ipv6(), false)?;
        if let Some(bbc) = self.bind_before_connecting {
            debug!("Using bind before connect");
            s.bind(bbc)?;
        }
        let ss = s.connect(addr).await?;
        stream_opts.apply_socket_opts(&ss, addr.is_ipv6())?;
        Ok(ss)
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

    pub fn apply_socket_opts(&self, s: &TcpStream, v6: bool) -> std::io::Result<()> {
        if let Some(v) = self.nodelay {
            debug!("Setting TCP_NODELAY");
            s.set_nodelay(v)?;
        }
        if let Some(v) = self.linger_s {
            debug!("Setting SO_LINGER");
            s.set_linger(Some(Duration::from_secs(v.into())))?;
        }
        if !v6 {
            if let Some(v) = self.ttl {
                debug!("Setting IP_TTL");
                s.set_ttl(v)?;
            }
        }

        let ss: SocketWrapper;
        #[cfg(any(unix, windows))]
        {
            ss = s.into();
        }
        #[cfg(not(any(unix, windows)))]
        {
            return Ok(());
        }

        if v6 {
            if let Some(v) = self.ttl {
                debug!("Setting IPV6_UNICAST_HOPS");
                ss.set_unicast_hops_v6(v)?;
            }
        }

        if v6 {
            if let Some(v) = self.tclass_v6 {
                debug!("Setting IPV6_TCLASS");
                cfg_gated_block_or_err!(
                    "tclass_v6",
                    #[cfg(any(
                        target_os = "android",
                        target_os = "dragonfly",
                        target_os = "freebsd",
                        target_os = "fuchsia",
                        target_os = "linux",
                        target_os = "macos",
                        target_os = "netbsd",
                        target_os = "openbsd"
                    ))],
                    {
                        ss.set_tclass_v6(v)?;
                    },
                );
            }
        } else if let Some(v) = self.tos_v4 {
            debug!("Setting IP_TOS");
            cfg_gated_block_or_err!(
                "tos_v4",
                #[cfg(not(any(
                    target_os = "fuchsia",
                    target_os = "redox",
                    target_os = "solaris",
                    target_os = "illumos",
                    target_os = "haiku",
                )))],
                {
                    ss.set_tos_v4(v)?;
                },
            );
        }

        if self.out_of_band_inline {
            debug!("Setting SO_OOBINLINE");
            #[cfg(not(target_os = "redox"))]
            ss.set_out_of_band_inline(true)?;
        }

        if let Some(v) = &self.tcp_congestion {
            debug!("Setting TCP_CONGESTION");
            cfg_gated_block_or_err!(
                "tcp_congestion",
                #[cfg(any(target_os = "freebsd", target_os = "linux"))],
                {
                    ss.set_tcp_congestion(v.as_bytes())?;
                },
            );
        }
        if let Some(v) = self.cpu_affinity {
            debug!("Setting SO_INCOMING_CPU");
            cfg_gated_block_or_err!(
                "cpu_affinity",
                #[cfg(target_os = "linux")],
                {
                    ss.set_cpu_affinity(v)?;
                },
            );
        }
        if let Some(v) = self.user_timeout_s {
            debug!("Setting TCP_USER_TIMEOUT");
            cfg_gated_block_or_err!(
                "user_timeout_s",
                #[cfg(any(
                    target_os = "android",
                    target_os = "fuchsia",
                    target_os = "linux",
                    target_os = "cygwin",
                ))],
                {
                    ss.set_tcp_user_timeout(Some(Duration::from_secs(v.into())))?;
                },
            );
        }
        if let Some(v) = self.priority {
            debug!("Setting SO_PRIORITY");
            cfg_gated_block_or_err!(
                "priority",
                #[cfg(any(target_os = "linux", target_os = "android", target_os = "fuchsia"))],
                {
                    ss.set_priority(v)?;
                },
            );
        }
        if let Some(v) = self.recv_buffer_size {
            debug!("Setting SO_RCVBUF");
            ss.set_recv_buffer_size(v)?;
        }
        if let Some(v) = self.send_buffer_size {
            debug!("Setting SO_SNDBUF");
            ss.set_send_buffer_size(v)?;
        }
        if let Some(v) = self.mss {
            debug!("Setting TCP_MAXSEG");
            cfg_gated_block_or_err!(
                "mss",
                #[cfg(all(unix, not(target_os = "redox")))],
                {
                    ss.set_tcp_mss(v)?;
                },
            );
        }
        if let Some(v) = self.mark {
            debug!("Setting SO_MARK");
            cfg_gated_block_or_err!(
                "mark",
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                {
                    ss.set_mark(v)?;
                },
            );
        }
        if let Some(v) = self.thin_linear_timeouts {
            debug!("Setting TCP_THIN_LINEAR_TIMEOUTS");
            cfg_gated_block_or_err!(
                "thin_linear_timeouts",
                #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))],
                {
                    ss.set_tcp_thin_linear_timeouts(v)?;
                },
            );
        }
        if let Some(v) = self.notsent_lowat {
            debug!("Setting TCP_NOTSENT_LOWAT");
            cfg_gated_block_or_err!(
                "notsent_lowat",
                #[cfg(any(target_os = "android", target_os = "linux"))],
                {
                    ss.set_tcp_notsent_lowat(v)?;
                },
            );
        }

        if let Some(v) = self.keepalive {
            debug!("Setting SO_KEEPALIVE");
            if v {
                #[allow(unused_mut)]
                let mut ka = socket2::TcpKeepalive::new();

                if let Some(w) = self.keepalive_interval_s {
                    cfg_gated_block_or_err!(
                        "keepalive_interval_s",
                        #[cfg(any(
                            target_os = "android",
                            target_os = "dragonfly",
                            target_os = "freebsd",
                            target_os = "fuchsia",
                            target_os = "illumos",
                            target_os = "ios",
                            target_os = "visionos",
                            target_os = "linux",
                            target_os = "macos",
                            target_os = "netbsd",
                            target_os = "tvos",
                            target_os = "watchos",
                            target_os = "windows",
                            target_os = "cygwin",
                        ))],
                        {
                            ka = ka.with_interval(Duration::from_secs(w.into()));
                        },
                    );
                }

                if let Some(w) = self.keepalive_idletime_s {
                    ka = ka.with_time(Duration::from_secs(w.into()));
                }

                if let Some(w) = self.keepalive_retries {
                    cfg_gated_block_or_err!(
                        "keepalive_interval_s",
                        #[cfg(any(
                            target_os = "android",
                            target_os = "dragonfly",
                            target_os = "freebsd",
                            target_os = "fuchsia",
                            target_os = "illumos",
                            target_os = "ios",
                            target_os = "visionos",
                            target_os = "linux",
                            target_os = "macos",
                            target_os = "netbsd",
                            target_os = "tvos",
                            target_os = "watchos",
                            target_os = "cygwin",
                            target_os = "windows",
                        ))],
                        {
                            ka = ka.with_retries(w);
                        },
                    );
                }

                ss.set_tcp_keepalive(&ka)?;
            } else {
                ss.set_keepalive(false)?;
            }
        }

        Ok(())
    }
}
