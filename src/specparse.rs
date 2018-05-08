use websocket::client::Url;
use super::{Specifier,Result};
use std::rc::Rc;

pub fn ws_l_prefix(s:&str) -> Option<&str> {
    if    s.starts_with("ws-l:") 
       || s.starts_with("l-ws:")
    {
        Some(&s[5..])
    }
    else if  s.starts_with("ws-listen:")
          || s.starts_with("listen-ws:")
    {
        Some(&s[10..])
    } else {
        None
    }
}

pub fn ws_c_prefix(s:&str) -> Option<&str> {
    if    s.starts_with("ws-c:") 
       || s.starts_with("c-ws:")
    {
        Some(&s[5..])
    }
    else if  s.starts_with("ws-connect:")
          || s.starts_with("connect-ws:")
    {
        Some(&s[11..])
    } else {
        None
    }
}

pub fn reuser_prefix(s:&str) -> Option<&str> {
    if s.starts_with("reuse:") {
        Some(&s[6..])
    } else {
        None
    }
}

pub fn ws_url_prefix(s:&str) -> Option<&str> {
    if s.starts_with("ws://") {
        Some(s)
    } else
    if s.starts_with("wss://") {
        Some(s)
    } else {
        None
    }
}

pub fn unix_specific_prefix(s:&str) -> Option<&str> {
    if s.starts_with("unix") {
        Some(s)
    } else 
    if s.starts_with("abstract") {
        #[cfg(not(linux))] {
            warn!("Abstract-namespaced UNIX sockets are unlikely to be supported here");
        }
        Some(s)
    } else {
        None
    }
}

pub fn boxup<T:Specifier+'static>(x:T) -> Result<Rc<Specifier>> {
    Ok(Rc::new(x))
}

pub fn spec(s : &str) -> Result<Rc<Specifier>>  {
    Specifier::from_str(s)
}

impl Specifier {
    fn from_str(s: &str) -> Result<Rc<Specifier>> {
        if s == "-" || s == "inetd:" || s == "stdio:" {
            #[cfg(all(unix,not(feature="no_unix_stdio")))]
            {
                boxup(super::stdio_peer::Stdio)
            }
            #[cfg(any(not(unix),feature="no_unix_stdio"))]
            {
                boxup(super::stdio_threaded_peer::ThreadedStdio)
            }
        } else 
        if s == "threadedstdio:" {
            boxup(super::stdio_threaded_peer::ThreadedStdio)
        } else
        if s == "mirror:" {
            boxup(super::mirror_peer::Mirror)
        } else
        if s == "clogged:" {
            boxup(super::trivial_peer::Clogged)
        } else
        if s.starts_with("literal:"){
            boxup(super::trivial_peer::Literal(s[8..].as_bytes().to_vec()))
        } else
        if s.starts_with("literalreply:"){
            boxup(super::mirror_peer::LiteralReply(s[13..].as_bytes().to_vec()))
        } else
        if s.starts_with("assert:"){
            boxup(super::trivial_peer::Assert(s[7..].as_bytes().to_vec()))
        } else
        if s.starts_with("tcp:") {
            boxup(super::net_peer::TcpConnect(s[4..].parse()?))
        } else 
        if s.starts_with("tcp-connect:") {
            boxup(super::net_peer::TcpConnect(s[12..].parse()?))
        } else 
        if s.starts_with("connect-tcp:") {
            boxup(super::net_peer::TcpConnect(s[12..].parse()?))
        } else 
        if s.starts_with("c-tcp:") {
            boxup(super::net_peer::TcpConnect(s[6..].parse()?))
        } else 
        if s.starts_with("tcp-c:") {
            boxup(super::net_peer::TcpConnect(s[6..].parse()?))
        } else 
        if s.starts_with("tcp-l:") {
            boxup(super::net_peer::TcpListen(s[6..].parse()?))
        } else 
        if s.starts_with("l-tcp:") {
            boxup(super::net_peer::TcpListen(s[6..].parse()?))
        } else 
        if s.starts_with("tcp-listen:") {
            boxup(super::net_peer::TcpListen(s[11..].parse()?))
        } else
        if s.starts_with("listen-tcp:") {
            boxup(super::net_peer::TcpListen(s[11..].parse()?))
        } else
        if s.starts_with("udp:") {
            boxup(super::net_peer::UdpConnect(s[4..].parse()?))
        } else
        if s.starts_with("udp-connect:") {
            boxup(super::net_peer::UdpConnect(s[12..].parse()?))
        } else
        if s.starts_with("connect-udp:") {
            boxup(super::net_peer::UdpConnect(s[12..].parse()?))
        } else
        if s.starts_with("udp-c:") {
            boxup(super::net_peer::UdpConnect(s[6..].parse()?))
        } else
        if s.starts_with("c-udp:") {
            boxup(super::net_peer::UdpConnect(s[6..].parse()?))
        } else
        if s.starts_with("udp-listen:") {
            boxup(super::net_peer::UdpListen(s[11..].parse()?))
        } else
        if s.starts_with("listen-udp:") {
            boxup(super::net_peer::UdpListen(s[11..].parse()?))
        } else
        if s.starts_with("udp-l:") {
            boxup(super::net_peer::UdpListen(s[6..].parse()?))
        } else
        if s.starts_with("l-udp:") {
            boxup(super::net_peer::UdpListen(s[6..].parse()?))
        } else
        if let Some(x) = ws_l_prefix(s) {
            if x == "" {
                Err("Specify underlying protocol for ws-l:")?;
            }
            if let Some(c) = x.chars().next() {
                if c.is_numeric() || c == '[' {
                    // Assuming user uses old format like ws-l:127.0.0.1:8080
                    return spec(&("ws-l:tcp-l:".to_owned() + x));
                }
            }
            boxup(super::ws_server_peer::WsUpgrade(spec(x)?))
        } else
        if let Some(x) = ws_c_prefix(s) {
            // TODO: de-hardcode URI
            boxup(super::ws_client_peer::WsConnect(spec(x)?, Url::parse("ws://0.0.0.0/").unwrap()))
        } else
        if let Some(x) = reuser_prefix(s) {
            boxup(super::connection_reuse_peer::Reuser(spec(x)?))
        } else
        if let Some(url_s) = ws_url_prefix(s) {
            let url : Url = url_s.parse()?;
            boxup(super::ws_client_peer::WsClient(url))
        } else 
        if s.starts_with("autoreconnect:") {
            boxup(super::reconnect_peer::AutoReconnect(spec(&s[14..])?))
        } else 
        if s.starts_with("open:") {
            return Err("There is no `open:` specifier. Consider `open-async:` or `readfile:` or `writefile:`")?;
        } else
        if s.starts_with("open-async:") {
            #[cfg(all(unix,not(feature="no_unix_stdio")))]
            {
                boxup(super::stdio_peer::OpenAsync(s[11..].into()))
            }
            #[cfg(any(not(unix),feature="no_unix_stdio"))]
            {
                Err("`open-async:` is not supported in this Websocat build")?;
            }
        } else
        if s.starts_with("open-fd:") {
            #[cfg(all(unix,not(feature="no_unix_stdio")))]
            {
                boxup(super::stdio_peer::OpenFdAsync(s[8..].parse()?))
            }
            #[cfg(any(not(unix),feature="no_unix_stdio"))]
            {
                Err("`open-fd:` is not supported in this Websocat build")?;
            }
        } else
        if s.starts_with("readfile:") {
            boxup(super::file_peer::ReadFile(s[9..].into()))
        } else
        if s.starts_with("writefile:") {
            boxup(super::file_peer::WriteFile(s[10..].into()))
        } else
        if let Some(ss) = unix_specific_prefix(s) {
            #[cfg(unix)]
            {
                if ss.starts_with("unix:") {
                    boxup(super::unix_peer::UnixConnect(s[5..].into()))
                } else
                if ss.starts_with("unix-connect:") {
                    boxup(super::unix_peer::UnixConnect(s[13..].into()))
                } else
                if ss.starts_with("connect-unix:") {
                    boxup(super::unix_peer::UnixConnect(s[13..].into()))
                } else
                if ss.starts_with("unix-c:") {
                    boxup(super::unix_peer::UnixConnect(s[7..].into()))
                } else
                if ss.starts_with("c-unix:") {
                    boxup(super::unix_peer::UnixConnect(s[7..].into()))
                } else
                if ss.starts_with("unix-listen:") {
                    boxup(super::unix_peer::UnixListen(s[12..].into()))
                } else
                if ss.starts_with("unix-l:") {
                    boxup(super::unix_peer::UnixListen(s[7..].into()))
                } else
                if ss.starts_with("listen-unix:") {
                    boxup(super::unix_peer::UnixListen(s[12..].into()))
                } else
                if ss.starts_with("l-unix:") {
                    boxup(super::unix_peer::UnixListen(s[7..].into()))
                } else
                if ss.starts_with("unix-dgram:") || ss.starts_with("dgram-unix:") {
                    let splits : Vec<&str> = s[11..].split(":").collect();
                    if splits.len() != 2 {
                        Err("Expected two colon-separted paths")?;
                    }
                    boxup(super::unix_peer::UnixDgram(splits[0].into(),splits[1].into()))
                } else
                if ss.starts_with("abstract:") {
                    boxup(super::unix_peer::AbstractConnect(s[9..].into()))
                } else
                if ss.starts_with("abstract-connect:") {
                    boxup(super::unix_peer::AbstractConnect(s[17..].into()))
                } else
                if ss.starts_with("connect-abstract:") {
                    boxup(super::unix_peer::AbstractConnect(s[17..].into()))
                } else
                if ss.starts_with("abstract-c:") {
                    boxup(super::unix_peer::AbstractConnect(s[11..].into()))
                } else
                if ss.starts_with("c-abstract:") {
                    boxup(super::unix_peer::AbstractConnect(s[11..].into()))
                } else
                if ss.starts_with("abstract-listen:") {
                    boxup(super::unix_peer::AbstractListen(s[16..].into()))
                } else
                if ss.starts_with("abstract-l:") {
                    boxup(super::unix_peer::AbstractListen(s[11..].into()))
                } else
                if ss.starts_with("listen-abstract:") {
                    boxup(super::unix_peer::AbstractListen(s[16..].into()))
                } else
                if ss.starts_with("l-abstract:") {
                    boxup(super::unix_peer::AbstractListen(s[11..].into()))
                } else
                if ss.starts_with("abstract-dgram:") || ss.starts_with("dgram-abstract:") {
                    let splits : Vec<&str> = s[15..].split(":").collect();
                    if splits.len() != 2 {
                        Err("Expected two colon-separted addresses")?;
                    }
                    boxup(super::unix_peer::AbstractDgram(splits[0].into(),splits[1].into()))
                } else
                {
                    error!("Invalid specifier string `{}`", ss);
                    Err("Wrong specifier")?
                }
            }
            #[cfg(not(unix))]
            {
                Err("`unix*:` or `abstract*:` are not supported in this Websocat build")?;
            }
        } else
        if s == "inetd-ws:" {
            return spec("ws-l:inetd:");
        } else {
            error!("Invalid specifier string `{}`", s);
            Err("Wrong specifier")?
        }
    }
}
