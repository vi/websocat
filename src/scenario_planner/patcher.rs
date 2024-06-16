use std::net::{IpAddr, SocketAddr};

use http::Uri;

use super::types::{CopyingType, Endpoint, Overlay, SpecifierStack, WebsocatInvocation};



impl WebsocatInvocation {
    pub fn get_copying_type(&self) -> CopyingType {
        match (self.left.get_copying_type(), self.right.get_copying_type()) {
            (CopyingType::ByteStream, CopyingType::ByteStream) => CopyingType::ByteStream,
            (CopyingType::Datarams, CopyingType::Datarams) => CopyingType::Datarams,
            _ => panic!("Incompatible types encountered: bytestream-oriented and datagram-oriented"),
        }
    }
}

impl SpecifierStack {
    fn get_copying_type(&self) -> CopyingType {
        let mut typ = self.innermost.get_copying_type();
        for ovl in &self.overlays {
            match ovl {
                Overlay::WsUpgrade(_) => typ = CopyingType::ByteStream,
                Overlay::WsFramer{..} => typ = CopyingType::Datarams,
                Overlay::StreamChunks => typ = CopyingType::Datarams,
            }
        }
        typ
    }
}

impl Endpoint {
    fn get_copying_type(&self) -> CopyingType {
        match self {
            Endpoint::TcpConnectByIp(_) => CopyingType::ByteStream,
            Endpoint::TcpListen(_) => CopyingType::ByteStream,
            Endpoint::WsUrl(_) => CopyingType::Datarams,
            Endpoint::WssUrl(_) => CopyingType::Datarams,
            Endpoint::Stdio => CopyingType::ByteStream,
            Endpoint::UdpConnect(_) => CopyingType::Datarams,
            Endpoint::UdpBind(_) => CopyingType::Datarams,
        }
    }
}

impl WebsocatInvocation {
    fn maybe_insert_chunker(&mut self) {
        match (self.left.get_copying_type(), self.right.get_copying_type()) {
            (CopyingType::ByteStream, CopyingType::ByteStream) => (),
            (CopyingType::Datarams, CopyingType::Datarams) => (),
            (CopyingType::ByteStream, CopyingType::Datarams) => {
                if self.opts.binary {
                    self.left.overlays.push(Overlay::StreamChunks);
                } else {
                    todo!()
                }
            }
            (CopyingType::Datarams, CopyingType::ByteStream) => {
                if self.opts.binary {
                    self.right.overlays.push(Overlay::StreamChunks);
                } else {
                    todo!()
                }
            }
        }
        assert_eq!(self.left.get_copying_type(), self.right.get_copying_type());
    }

    pub fn patches(&mut self) -> anyhow::Result<()> {
        self.left.maybe_splitup_ws_endpoint()?;
        self.right.maybe_splitup_ws_endpoint()?;
        self.maybe_insert_chunker();
        Ok(())
    }
}

impl SpecifierStack {
    fn maybe_splitup_ws_endpoint(&mut self) -> anyhow::Result<()> {
        if let Endpoint::WsUrl(ref u) = self.innermost {
            let mut parts = u.clone().into_parts();
            let auth = parts.authority.take().unwrap();
            let (mut host, port) = (auth.host(), auth.port_u16().unwrap_or(80));

            if host.starts_with('[') && host.ends_with(']') {
                host = host.strip_prefix('[').unwrap().strip_suffix(']').unwrap();
            }

            let Ok(ip) : Result<IpAddr, _> = host.parse() else {
                anyhow::bail!("Hostnames not supported yet")
            };

            let addr = SocketAddr::new(ip, port);

            self.innermost = Endpoint::TcpConnectByIp(addr);

            parts.scheme = None;
            let newurl = Uri::from_parts(parts).unwrap();

            self.overlays.insert(0, Overlay::WsUpgrade(newurl));
            self.overlays.insert(1, Overlay::WsFramer { client_mode: true });
            

        }
        Ok(())
    }
}
