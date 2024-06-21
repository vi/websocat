use std::net::{IpAddr, SocketAddr};

use http::Uri;
use tracing::warn;

use super::{
    types::{
        CopyingType, Endpoint, Overlay, PreparatoryAction, SpecifierStack, WebsocatInvocation,
    },
    utils::IdentifierGenerator,
};

impl WebsocatInvocation {
    pub fn patches(&mut self, vars: &mut IdentifierGenerator) -> anyhow::Result<()> {
        self.left.maybe_splitup_client_ws_endpoint()?;
        self.right.maybe_splitup_client_ws_endpoint()?;
        self.left.maybe_splitup_server_ws_endpoint()?;
        self.right.maybe_splitup_server_ws_endpoint()?;
        if !self.opts.late_resolve {
            self.left.maybe_early_resolve(&mut self.beginning, vars);
            self.right.maybe_early_resolve(&mut self.beginning, vars);
        }
        self.maybe_fill_in_tls_details(vars)?;
        self.maybe_insert_chunker();
        Ok(())
    }

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

    fn maybe_fill_in_tls_details(&mut self, vars: &mut IdentifierGenerator) -> anyhow::Result<()> {
        if let Some(ref d) = self.opts.tls_domain {
            let mut patch_occurred = false;
            for x in self
                .left
                .overlays
                .iter_mut()
                .chain(self.right.overlays.iter_mut())
            {
                match x {
                    Overlay::TlsClient { domain, .. } => {
                        if domain != d {
                            *domain = d.clone();
                            patch_occurred = true;
                        }
                    }
                    _ => (),
                }
            }
            if !patch_occurred {
                warn!("--tls-domain option did not affect anything");
            }
        }

        let mut need_to_insert_context = false;
        for x in self.left.overlays.iter().chain(self.right.overlays.iter()) {
            match x {
                Overlay::TlsClient {
                    varname_for_connector,
                    ..
                } if varname_for_connector.is_empty() => need_to_insert_context = true,
                _ => (),
            }
        }
        if !need_to_insert_context {
            return Ok(());
        }
        let varname_for_connector = vars.getnewvarname("tlsctx");
        let v = varname_for_connector.clone();
        self.beginning.push(PreparatoryAction::CreateTlsConnector {
            varname_for_connector,
        });
        for x in self
            .left
            .overlays
            .iter_mut()
            .chain(self.right.overlays.iter_mut())
        {
            match x {
                Overlay::TlsClient {
                    varname_for_connector,
                    ..
                } if varname_for_connector.is_empty() => {
                    *varname_for_connector = v.clone();
                }
                _ => (),
            }
        }
        Ok(())
    }
}

impl SpecifierStack {
    fn maybe_splitup_client_ws_endpoint(&mut self) -> anyhow::Result<()> {
        match self.innermost {
            Endpoint::WsUrl(ref u) | Endpoint::WssUrl(ref u) => {
                let mut parts = u.clone().into_parts();
                let auth = parts.authority.take().unwrap();
                if auth.as_str().contains('@') {
                    anyhow::bail!("Usernames in URLs not supported");
                }
                let wss_mode = match self.innermost {
                    Endpoint::WsUrl(_) => false,
                    Endpoint::WssUrl(_) => true,
                    _ => unreachable!(),
                };
                let default_port = if wss_mode { 443 } else { 80 };
                let (mut host, port) = (auth.host(), auth.port_u16().unwrap_or(default_port));

                if host.starts_with('[') && host.ends_with(']') {
                    host = host.strip_prefix('[').unwrap().strip_suffix(']').unwrap();
                }

                let ip: Result<IpAddr, _> = host.parse();

                match ip {
                    Ok(ip) => {
                        let addr = SocketAddr::new(ip, port);

                        self.innermost = Endpoint::TcpConnectByIp(addr);
                    }
                    Err(_) => {
                        self.innermost = Endpoint::TcpConnectByLateHostname {
                            hostname: format!("{host}:{port}"),
                        };
                    }
                };

                parts.scheme = None;
                let newurl = Uri::from_parts(parts).unwrap();

                self.overlays.insert(
                    0,
                    Overlay::WsUpgrade {
                        uri: newurl,
                        host: auth.to_string(),
                    },
                );
                self.overlays
                    .insert(1, Overlay::WsFramer { client_mode: true });

                if wss_mode {
                    self.overlays.insert(
                        0,
                        Overlay::TlsClient {
                            domain: host.to_owned(),
                            varname_for_connector: String::new(),
                        },
                    );
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn maybe_splitup_server_ws_endpoint(&mut self) -> anyhow::Result<()> {
        match self.innermost {
            Endpoint::WsListen(a) => {
                self.innermost = Endpoint::TcpListen(a);

                self.overlays.insert(
                    0,
                    Overlay::WsAccept {  },
                );
                self.overlays
                    .insert(1, Overlay::WsFramer { client_mode: false });
            }
            _ => (),
        }
        Ok(())
    }

    fn maybe_early_resolve(
        &mut self,
        beginning: &mut Vec<PreparatoryAction>,
        vars: &mut IdentifierGenerator,
    ) {
        match &self.innermost {
            Endpoint::TcpConnectByLateHostname { hostname } => {
                let varname_for_addrs = vars.getnewvarname("addrs");
                beginning.push(PreparatoryAction::ResolveHostname {
                    hostname: hostname.clone(),
                    varname_for_addrs: varname_for_addrs.clone(),
                });
                self.innermost = Endpoint::TcpConnectByEarlyHostname { varname_for_addrs };
            }
            _ => (),
        }
    }
}

impl WebsocatInvocation {
    pub fn get_copying_type(&self) -> CopyingType {
        match (self.left.get_copying_type(), self.right.get_copying_type()) {
            (CopyingType::ByteStream, CopyingType::ByteStream) => CopyingType::ByteStream,
            (CopyingType::Datarams, CopyingType::Datarams) => CopyingType::Datarams,
            _ => {
                panic!("Incompatible types encountered: bytestream-oriented and datagram-oriented")
            }
        }
    }
}

impl SpecifierStack {
    fn get_copying_type(&self) -> CopyingType {
        let mut typ = self.innermost.get_copying_type();
        for ovl in &self.overlays {
            match ovl {
                Overlay::WsUpgrade { .. } => typ = CopyingType::ByteStream,
                Overlay::WsFramer { .. } => typ = CopyingType::Datarams,
                Overlay::StreamChunks => typ = CopyingType::Datarams,
                Overlay::TlsClient { .. } => typ = CopyingType::ByteStream,
                Overlay::WsAccept {  } => typ = CopyingType::ByteStream,
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
            Endpoint::TcpConnectByEarlyHostname { .. } => CopyingType::ByteStream,
            Endpoint::TcpConnectByLateHostname { hostname: _ } => CopyingType::ByteStream,
            Endpoint::WsUrl(_) => CopyingType::Datarams,
            Endpoint::WssUrl(_) => CopyingType::Datarams,
            Endpoint::Stdio => CopyingType::ByteStream,
            Endpoint::UdpConnect(_) => CopyingType::Datarams,
            Endpoint::UdpBind(_) => CopyingType::Datarams,
            Endpoint::WsListen(_) => CopyingType::Datarams,
        }
    }
}
