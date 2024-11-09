use std::{ffi::OsStr, net::SocketAddr};
use clap_lex::OsStrExt;

use super::{
    types::{Endpoint, Overlay, SpecifierStack},
    utils::StripPrefixMany,
};

impl SpecifierStack {
    pub fn from_str(mut x: &OsStr) -> anyhow::Result<SpecifierStack> {
        let innermost;
        let mut overlays = vec![];

        loop {
            match ParseStrChunkResult::from_str(x)? {
                ParseStrChunkResult::Endpoint(e) => {
                    innermost = e;
                    break;
                }
                ParseStrChunkResult::Overlay { ovl, rest } => {
                    overlays.push(ovl);
                    x = rest;
                }
            }
        }

        overlays.reverse();

        Ok(SpecifierStack {
            innermost: innermost,
            overlays,
        })
    }
}

enum ParseStrChunkResult<'a> {
    Endpoint(Endpoint),
    Overlay { ovl: Overlay, rest: &'a OsStr },
}

impl ParseStrChunkResult<'_> {
    fn from_str(x: &OsStr) -> anyhow::Result<ParseStrChunkResult<'_>> {
        if x.starts_with("ws://") {
            let s : &str = x.try_into()?;
            let u = http::Uri::try_from(s)?;
            if u.authority().is_none() {
                anyhow::bail!("ws:// URL without authority");
            }
            Ok(ParseStrChunkResult::Endpoint(Endpoint::WsUrl(u)))
        } else if x.starts_with("wss://") {
            let s : &str = x.try_into()?;
            let u = http::Uri::try_from(s)?;
            if u.authority().is_none() {
                anyhow::bail!("wss:// URL without authority");
            }
            Ok(ParseStrChunkResult::Endpoint(Endpoint::WssUrl(u)))
        } else if let Some(rest) =
            x.strip_prefix_many(&["tcp:", "tcp-connect:", "connect-tcp:", "tcp-c:", "c-tcp:"])
        {
            let s : &str = rest.try_into()?;
            let a: Result<SocketAddr, _> = s.parse();
            match a {
                Ok(a) => Ok(ParseStrChunkResult::Endpoint(Endpoint::TcpConnectByIp(a))),
                Err(_) => Ok(ParseStrChunkResult::Endpoint(
                    Endpoint::TcpConnectByLateHostname {
                        hostname: s.to_owned(),
                    },
                )),
            }
        } else if let Some(rest) =
            x.strip_prefix_many(&["tcp-listen:", "listen-tcp:", "tcp-l:", "l-tcp:"])
        {
            let s : &str = rest.try_into()?;            
            let a: SocketAddr = s.parse()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::TcpListen(a)))
        } else if x == "-" {
            Ok(ParseStrChunkResult::Endpoint(Endpoint::Stdio))
        } else if let Some(rest) = x.strip_prefix("stdio:") {
            if !rest.is_empty() {
                anyhow::bail!("stdio: endpoint does not take any argument");
            }
            Ok(ParseStrChunkResult::Endpoint(Endpoint::Stdio))
        } else if let Some(rest) = x.strip_prefix("tls:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::TlsClient {
                    domain: String::new(),
                    varname_for_connector: String::new(),
                },
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("ws-accept:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WsAccept {},
                rest,
            })
        } else if let Some(rest) = x.strip_prefix_many(&["ws-connect:", "connect-ws:", "ws-c:", "c-ws:"]) {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WsClient {},
                rest,
            })
        } else if let Some(rest) = x.strip_prefix_many(&["ws-upgrade:", "upgrade-ws:", "ws-u:", "u-ws:"]) {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WsServer {},
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("ws-ll-client:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WsFramer { client_mode: true },
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("ws-ll-server:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WsFramer { client_mode: false },
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("ws-l:") {
            let s : &str = rest.try_into()?;
            let a: SocketAddr = s.parse()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::WsListen(a)))
        } else if let Some(rest) = x.strip_prefix("log:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::Log {
                    // real value is filled in in the patcher
                    datagram_mode: false,
                },
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("read_chunk_limiter:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::ReadChunkLimiter,
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("write_chunk_limiter:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WriteChunkLimiter,
                rest,
            })
        } else if let Some(rest) = x.strip_prefix("write_buffer:") {
            Ok(ParseStrChunkResult::Overlay {
                ovl: Overlay::WriteBuffer,
                rest,
            })
        } else if let Some(rest) =
            x.strip_prefix_many(&["udp:", "udp-connect:", "connect-udp:", "udp-c:", "c-udp:"])
        {
            let s : &str = rest.try_into()?;
            let a: SocketAddr = s.parse()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::UdpConnect(a)))
        } else if let Some(rest) = x.strip_prefix_many(&[
            "udp-bind:",
            "bind-udp:",
            "udp-listen:",
            "listen-udp:",
            "udp-l:",
            "l-udp:",
        ]) {
            let s : &str = rest.try_into()?;
            let a: SocketAddr = s.parse()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::UdpBind(a)))
        } else if let Some(rest) = x.strip_prefix("udp-server:") {
            let s : &str = rest.try_into()?;
            let a: SocketAddr = s.parse()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::UdpServer(a)))
        } else if let Some(rest) = x.strip_prefix("exec:") {
            Ok(ParseStrChunkResult::Endpoint(Endpoint::Exec(
                rest.to_owned(),
            )))
        } else if let Some(rest) = x.strip_prefix_many(&["cmd:", "sh-c:"]) {
            Ok(ParseStrChunkResult::Endpoint(Endpoint::Cmd(
                rest.to_owned(),
            )))
        } else if let Some(rest) =
            x.strip_prefix_many(&["empty:", "null:", "dummy-datagrams:", "dummy:"])
        {
            if !rest.is_empty() {
                anyhow::bail!("empty: endpoint does not take any argument");
            }
            Ok(ParseStrChunkResult::Endpoint(Endpoint::DummyDatagrams))
        } else if let Some(rest) = x.strip_prefix_many(&["devnull:", "dummy-stream:"]) {
            if !rest.is_empty() {
                anyhow::bail!("devnull: endpoint does not take any argument");
            }
            Ok(ParseStrChunkResult::Endpoint(Endpoint::DummyStream))
        } else if let Some(rest) = x.strip_prefix("literal:") {
            let s : &str = rest.try_into()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::Literal(
                s.to_owned(),
            )))
        } else if let Some(rest) = x.strip_prefix("literal-base64:") {
            let s : &str = rest.try_into()?;
            Ok(ParseStrChunkResult::Endpoint(Endpoint::LiteralBase64(
                s.to_owned(),
            )))
        } else if let Some(rest) = x.strip_prefix_many(&["unix:", "unix-connect:", "connect-unix:", "unix-c:", "c-unix:"]) {
            Ok(ParseStrChunkResult::Endpoint(Endpoint::UnixConnect(
                rest.to_owned(),
            )))
        } else if let Some(rest) = x.strip_prefix_many(&["unix-listen:", "listen-unix:", "unix-l:", "l-unix:"]) {
            Ok(ParseStrChunkResult::Endpoint(Endpoint::UnixListen (
                rest.to_owned(),
            )))
        } else {
            anyhow::bail!("Unknown specifier: {x:?}")
        }
    }
}
