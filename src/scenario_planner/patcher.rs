use std::net::{IpAddr, SocketAddr};

use http::{uri::Authority, Uri};
use tracing::{debug, warn};

use crate::cli::WebsocatArgs;

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
        self.left.maybe_splitup_ws_c_overlay(&self.opts)?;
        self.right.maybe_splitup_ws_c_overlay(&self.opts)?;
        self.left.maybe_splitup_ws_u_overlay(&self.opts)?;
        self.right.maybe_splitup_ws_u_overlay(&self.opts)?;
        self.left.maybe_splitup_server_ws_endpoint()?;
        self.right.maybe_splitup_server_ws_endpoint()?;
        if !self.opts.late_resolve {
            self.left.maybe_early_resolve(&mut self.beginning, vars);
            self.right.maybe_early_resolve(&mut self.beginning, vars);
        }
        self.maybe_fill_in_tls_details(vars)?;
        if self.opts.log_traffic {
            if !self.right.insert_log_overlay() {
                if !self.left.insert_log_overlay() {
                    warn!("Failed to automaticelly insert log: overlay");
                }
            }
        }
        self.left.fill_in_log_overlay_type();
        self.right.fill_in_log_overlay_type();
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
                    self.left.overlays.push(Overlay::LineChunks);
                }
            }
            (CopyingType::Datarams, CopyingType::ByteStream) => {
                if self.opts.binary {
                    self.right.overlays.push(Overlay::StreamChunks);
                } else {
                    self.right.overlays.push(Overlay::LineChunks);
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

struct MyUrlParts {
    auth: Option<Authority>,
    /// just the path part
    newurl: Uri,
}

impl MyUrlParts {
    fn process_url(u: &Uri) -> anyhow::Result<Self> {
        let mut parts = u.clone().into_parts();

        let auth = parts.authority.take();
        if let Some(ref auth) = auth {
            if auth.as_str().contains('@') {
                anyhow::bail!("Usernames in URLs not supported");
            }
        }

        parts.scheme = None;
        let mut newurl = Uri::from_parts(parts).unwrap();

        if newurl.path().is_empty() {
            debug!("Patching empty URL to be /");
            newurl = Uri::from_static("/");
        }
        Ok(MyUrlParts { auth, newurl })
    }
}


impl SpecifierStack {
    fn maybe_splitup_client_ws_endpoint(&mut self) -> anyhow::Result<()> {
        match self.innermost {
            Endpoint::WsUrl(ref u) | Endpoint::WssUrl(ref u) => {
                let MyUrlParts{auth, newurl} = MyUrlParts::process_url(u)?;

                // URI should be checked to be a full one in `fromstr.rs`.
                let auth = auth.unwrap();

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

                self.overlays.insert(
                    0,
                    Overlay::WsUpgrade {
                        uri: newurl,
                        host: Some(auth.to_string()),
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

    fn maybe_splitup_ws_c_overlay(&mut self, opts: &WebsocatArgs) -> anyhow::Result<()> {
        let Some((position_to_redact, _)) = self
            .overlays
            .iter()
            .enumerate()
            .find(|(_, ovl)| matches!(ovl, Overlay::WsClient))
        else {
            return Ok(());
        };

        let uri = Uri::try_from(opts.ws_c_uri.as_deref().unwrap_or("/"))?;
        let MyUrlParts{auth, newurl} = MyUrlParts::process_url(&uri)?;

        self.overlays.remove(position_to_redact);
        self.overlays.insert(
            position_to_redact,
            Overlay::WsUpgrade {
                uri: newurl,
                host: auth.map(|x|x.to_string()),
            },
        );
        self.overlays.insert(position_to_redact+1, Overlay::WsFramer { client_mode: true });

        Ok(())
    }

    fn maybe_splitup_ws_u_overlay(&mut self, _opts: &WebsocatArgs) -> anyhow::Result<()> {
        let Some((position_to_redact, _)) = self
            .overlays
            .iter()
            .enumerate()
            .find(|(_, ovl)| matches!(ovl, Overlay::WsServer))
        else {
            return Ok(());
        };

        self.overlays.remove(position_to_redact);
        self.overlays.insert(
            position_to_redact,
            Overlay::WsAccept {  }
        );
        self.overlays.insert(position_to_redact+1, Overlay::WsFramer { client_mode: false });

        Ok(())
    }

    fn maybe_splitup_server_ws_endpoint(&mut self) -> anyhow::Result<()> {
        match self.innermost {
            Endpoint::WsListen(a) => {
                self.innermost = Endpoint::TcpListen(a);

                self.overlays.insert(0, Overlay::WsAccept {});
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

    fn fill_in_log_overlay_type(&mut self) {
        let mut typ = self.innermost.get_copying_type();
        for ovl in &mut self.overlays {
            match ovl {
                Overlay::Log { datagram_mode } => {
                    *datagram_mode = typ == CopyingType::Datarams;
                }
                x => typ = x.get_copying_type(),
            }
        }
    }

    /// returns true if it was inserted (or `log:` already present)
    fn insert_log_overlay(&mut self) -> bool {
        let mut index = None;
        for (i, ovl) in self.overlays.iter().enumerate() {
            match ovl {
                Overlay::WsUpgrade { .. } => (),
                Overlay::WsAccept { .. } => (),
                Overlay::WsFramer { .. } => (),
                Overlay::TlsClient { .. } => {
                    index = Some(i);
                    break;
                }
                Overlay::StreamChunks => (),
                Overlay::Log { .. } => return true,
                Overlay::LineChunks => (),
                Overlay::WsClient => (),
                Overlay::WsServer => (),
                Overlay::ReadChunkLimiter => (),
                Overlay::WriteChunkLimiter  => (),
                Overlay::WriteBuffer => (),
            }
        }
        if let Some(i) = index {
            self.overlays.insert(
                i + 1,
                Overlay::Log {
                    datagram_mode: false,
                },
            );
            true
        } else {
            let do_insert = match &self.innermost {
                Endpoint::TcpConnectByEarlyHostname { .. } => true,
                Endpoint::TcpConnectByLateHostname { .. } => true,
                Endpoint::TcpConnectByIp(_) => true,
                Endpoint::TcpListen(_) => true,
                Endpoint::WsUrl(_) => false,
                Endpoint::WssUrl(_) => false,
                Endpoint::WsListen(_) => false,
                Endpoint::Stdio => false,
                Endpoint::UdpConnect(_) => true,
                Endpoint::UdpBind(_) => true,
                Endpoint::UdpServer(_) => true,
                Endpoint::Exec(_) => true,
                Endpoint::Cmd(_) => true,
                Endpoint::DummyStream => false,
                Endpoint::DummyDatagrams => false,
                Endpoint::Literal(_) => false,
                Endpoint::LiteralBase64(_) => false,
                Endpoint::UnixConnect(_) => true,
                Endpoint::UnixListen(_) => true,
            };
            if do_insert {
                // datagram mode may be patched later
                self.overlays.insert(
                    0,
                    Overlay::Log {
                        datagram_mode: false,
                    },
                )
            }
            do_insert
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
            typ = ovl.get_copying_type();
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
            Endpoint::UdpServer(_) => CopyingType::Datarams,
            Endpoint::Exec(_) => CopyingType::ByteStream,
            Endpoint::Cmd(_) => CopyingType::ByteStream,
            Endpoint::DummyStream => CopyingType::ByteStream,
            Endpoint::DummyDatagrams => CopyingType::Datarams,
            Endpoint::Literal(_) => CopyingType::ByteStream,
            Endpoint::LiteralBase64(_) => CopyingType::ByteStream,
            Endpoint::UnixConnect(_) => CopyingType::ByteStream,
            Endpoint::UnixListen(_) => CopyingType::ByteStream,
        }
    }
}

impl Overlay {
    fn get_copying_type(&self) -> CopyingType {
        match self {
            Overlay::WsUpgrade { .. } => CopyingType::ByteStream,
            Overlay::WsFramer { .. } => CopyingType::Datarams,
            Overlay::StreamChunks => CopyingType::Datarams,
            Overlay::LineChunks => CopyingType::Datarams,
            Overlay::TlsClient { .. } => CopyingType::ByteStream,
            Overlay::WsAccept {} => CopyingType::ByteStream,
            Overlay::Log { datagram_mode } => {
                if *datagram_mode {
                    CopyingType::Datarams
                } else {
                    CopyingType::ByteStream
                }
            }
            Overlay::WsClient => CopyingType::Datarams,
            Overlay::WsServer => CopyingType::Datarams,
            Overlay::ReadChunkLimiter  => CopyingType::ByteStream,
            Overlay::WriteChunkLimiter => CopyingType::ByteStream,
            Overlay::WriteBuffer => CopyingType::ByteStream,
        }
    }
}
