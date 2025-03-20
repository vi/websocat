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
        self.left.maybe_patch_existing_ws_request(&self.opts)?;
        self.right.maybe_patch_existing_ws_request(&self.opts)?;
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
        if self.opts.log_traffic
            && !self.right.insert_log_overlay()
            && !self.left.insert_log_overlay()
        {
            warn!("Failed to automaticelly insert log: overlay");
        }
        self.left.fill_in_log_overlay_type();
        self.right.fill_in_log_overlay_type();
        self.maybe_insert_chunker();
        self.left
            .maybe_process_writesplitoff(&mut self.write_splitoff)?;
        self.right
            .maybe_process_writesplitoff(&mut self.write_splitoff)?;
        self.maybe_insert_reuser();
        self.left.maybe_process_reuser(vars, &mut self.beginning)?;
        self.right.maybe_process_reuser(vars, &mut self.beginning)?;
        Ok(())
    }

    fn maybe_insert_chunker(&mut self) {
        if self.opts.exec_dup2.is_some() {
            // dup2 mode is speial, it ignores any overlays and directly forwards
            // file desciptor to process. messages vs bytestreams distinction does not matter in this mode.
            return;
        }

        // use Datagrams copying type (and auto-insert appropriate overlays) if at least one of things expects datagrams.

        let mut working_copying_type = self.left.get_copying_type();
        if self.right.get_copying_type().is_dgrms() {
            working_copying_type = CopyingType::Datarams;
        }

        /*if let Some(ref spl) = self.write_splitoff {
            if spl.get_copying_type().is_dgrms() {
                working_copying_type = CopyingType::Datarams;
            }
        }*/

        if working_copying_type == CopyingType::ByteStream {
            // everything is already ByteStream
            return;
        }

        let overlay_to_insert = || {
            if self.opts.binary {
                Overlay::StreamChunks
            } else {
                Overlay::LineChunks
            }
        };

        if self.left.get_copying_type().is_bstrm() {
            self.left.overlays.push(overlay_to_insert());
        }
        if self.right.get_copying_type().is_bstrm() {
            self.right.overlays.push(overlay_to_insert());
        }

        /*if let Some(ref mut spl) = self.write_splitoff {
            if spl.get_copying_type().is_bstrm() {
                spl.overlays.push(overlay_to_insert());
            }
        }*/
        assert_eq!(self.left.get_copying_type(), self.right.get_copying_type());
    }

    fn maybe_insert_reuser(&mut self) {
        if self.get_copying_type() == CopyingType::Datarams
            && self.left.is_multiconn(&self.opts)
            && self.right.prefers_being_single(&self.opts)
        {
            self.right.overlays.push(Overlay::SimpleReuser);
        }
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
                if let Overlay::TlsClient { domain, .. } = x {
                    if domain != d {
                        *domain = d.clone();
                        patch_occurred = true;
                    }
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
                let MyUrlParts { auth, newurl } = MyUrlParts::process_url(u)?;

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

    fn maybe_patch_existing_ws_request(&mut self, opts: &WebsocatArgs) -> anyhow::Result<()> {
        for ovl in &mut self.overlays {
            if let Overlay::WsUpgrade { uri, host } = ovl {
                if let Some(ref ws_c_uri) = opts.ws_c_uri {
                    if uri == "/" && host.is_none() {
                        let ws_c_uri = Uri::try_from(ws_c_uri)?;
                        let MyUrlParts { auth, newurl } = MyUrlParts::process_url(&ws_c_uri)?;
                        *uri = newurl;
                        *host = auth.map(|x| x.to_string());
                    }
                }
            }
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
        let MyUrlParts { auth, newurl } = MyUrlParts::process_url(&uri)?;

        self.overlays.remove(position_to_redact);
        self.overlays.insert(
            position_to_redact,
            Overlay::WsUpgrade {
                uri: newurl,
                host: auth.map(|x| x.to_string()),
            },
        );
        self.overlays.insert(
            position_to_redact + 1,
            Overlay::WsFramer { client_mode: true },
        );

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
        self.overlays
            .insert(position_to_redact, Overlay::WsAccept {});
        self.overlays.insert(
            position_to_redact + 1,
            Overlay::WsFramer { client_mode: false },
        );

        Ok(())
    }

    fn maybe_splitup_server_ws_endpoint(&mut self) -> anyhow::Result<()> {
        if let Endpoint::WsListen(a) = self.innermost {
            self.innermost = Endpoint::TcpListen(a);

            self.overlays.insert(0, Overlay::WsAccept {});
            self.overlays
                .insert(1, Overlay::WsFramer { client_mode: false });
        }
        Ok(())
    }

    fn maybe_early_resolve(
        &mut self,
        beginning: &mut Vec<PreparatoryAction>,
        vars: &mut IdentifierGenerator,
    ) {
        if let Endpoint::TcpConnectByLateHostname { hostname } = &self.innermost {
            let varname_for_addrs = vars.getnewvarname("addrs");
            beginning.push(PreparatoryAction::ResolveHostname {
                hostname: hostname.clone(),
                varname_for_addrs: varname_for_addrs.clone(),
            });
            self.innermost = Endpoint::TcpConnectByEarlyHostname { varname_for_addrs };
        }
    }

    fn fill_in_log_overlay_type(&mut self) {
        let mut typ = self.innermost.get_copying_type();
        for ovl in &mut self.overlays {
            match ovl {
                Overlay::Log { datagram_mode } => {
                    *datagram_mode = typ == CopyingType::Datarams;
                }
                x => {
                    if let Some(nct) = x.get_copying_type() {
                        typ = nct
                    }
                }
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
                Overlay::WriteChunkLimiter => (),
                Overlay::WriteBuffer => (),
                Overlay::LengthPrefixedChunks => (),
                Overlay::SimpleReuser => (),
                Overlay::WriteSplitoff => (),
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
                Endpoint::TcpListenFd(_) => true,
                Endpoint::TcpListenFdNamed(_) => true,
                Endpoint::WsUrl(_) => false,
                Endpoint::WssUrl(_) => false,
                Endpoint::WsListen(_) => false,
                Endpoint::Stdio => false,
                Endpoint::UdpConnect(_) => true,
                Endpoint::UdpBind(_) => true,
                Endpoint::UdpFd(_) => true,
                Endpoint::UdpFdNamed(_) => true,
                Endpoint::UdpServer(_) => true,
                Endpoint::UdpServerFd(_) => true,
                Endpoint::UdpServerFdNamed(_) => true,
                Endpoint::Exec(_) => true,
                Endpoint::Cmd(_) => true,
                Endpoint::DummyStream => false,
                Endpoint::DummyDatagrams => false,
                Endpoint::Literal(_) => false,
                Endpoint::LiteralBase64(_) => false,
                Endpoint::UnixConnect(_) => true,
                Endpoint::UnixListen(_) => true,
                Endpoint::AbstractConnect(_) => true,
                Endpoint::AbstractListen(_) => true,
                Endpoint::UnixListenFd(_) => true,
                Endpoint::UnixListenFdNamed(_) => true,
                Endpoint::SeqpacketConnect(_) => true,
                Endpoint::SeqpacketListen(_) => true,
                Endpoint::AbstractSeqpacketConnect(_) => true,
                Endpoint::AbstractSeqpacketListen(_) => true,
                Endpoint::SeqpacketListenFd(_) => true,
                Endpoint::SeqpacketListenFdNamed(_) => true,
                Endpoint::MockStreamSocket(_) => false,
                Endpoint::RegistryStreamListen(_) => false,
                Endpoint::RegistryStreamConnect(_) => false,
                Endpoint::AsyncFd(_) => true,
                Endpoint::SimpleReuserEndpoint(..) => false,
                Endpoint::ReadFile(..) => false,
                Endpoint::WriteFile(..) => false,
                Endpoint::AppendFile(..) => false,
                Endpoint::Random => false,
                Endpoint::Zero => false,
                Endpoint::WriteSplitoff { .. } => false,
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

    fn maybe_process_reuser(
        &mut self,
        vars: &mut IdentifierGenerator,
        beginnings: &mut Vec<PreparatoryAction>,
    ) -> anyhow::Result<()> {
        let mut the_index = None;
        for (i, ovl) in self.overlays.iter().enumerate() {
            match ovl {
                Overlay::SimpleReuser => {
                    the_index = Some(i);
                    break;
                }
                _ => (),
            }
        }

        if let Some(i) = the_index {
            let mut switcheroo = Box::new(SpecifierStack {
                innermost: Endpoint::DummyDatagrams, // temporary
                overlays: vec![],                    // temporary
                position: self.position,
            });
            std::mem::swap(self, &mut switcheroo);

            // Preserve overlays specified before `reuse:`
            self.overlays = Vec::from_iter(switcheroo.overlays.drain(i + 1..));
            // Remove `reuse:` overlay itself, now that it has been turned into an endpoint
            switcheroo.overlays.remove(i);

            let varname = vars.getnewvarname("reuser");
            beginnings.push(PreparatoryAction::CreateSimpleReuserListener {
                varname_for_reuser: varname.clone(),
            });
            self.innermost = Endpoint::SimpleReuserEndpoint(varname, switcheroo);

            // continue substituting other, nested reusers, if any
            self.maybe_process_reuser(vars, beginnings)?;
        }
        Ok(())
    }

    fn maybe_process_writesplitoff(
        &mut self,
        write_splitoff: &mut Option<SpecifierStack>,
    ) -> anyhow::Result<()> {
        let mut the_index = None;
        for (i, ovl) in self.overlays.iter().enumerate() {
            match ovl {
                Overlay::WriteSplitoff => {
                    the_index = Some(i);
                    break;
                }
                _ => (),
            }
        }

        let Some(the_index) = the_index else {
            return Ok(());
        };
        let Some(write_splitoff_stack) = write_splitoff.take() else {
            anyhow::bail!("`write-splitoff:` overlay specified without accompanying --write-splitoff option or specified multiple times")
        };

        let mut switcheroo = Box::new(SpecifierStack {
            innermost: Endpoint::DummyDatagrams, // temporary
            overlays: vec![],                    // temporary
            position: self.position,
        });
        std::mem::swap(self, &mut switcheroo);

        // Preserve overlays specified before `reuse:`
        self.overlays = Vec::from_iter(switcheroo.overlays.drain(the_index + 1..));
        // Remove `reuse:` overlay itself, now that it has been turned into an endpoint
        switcheroo.overlays.remove(the_index);

        self.innermost = Endpoint::WriteSplitoff {
            read: switcheroo,
            write: Box::new(write_splitoff_stack),
        };

        // continue processing deeper just to handle possible duplicate errors
        self.maybe_process_writesplitoff(&mut None)?;

        Ok(())
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
            if let Some(t) = ovl.get_copying_type() {
                typ = t
            }
        }
        typ
    }
}

impl Endpoint {
    pub(super) fn get_copying_type(&self) -> CopyingType {
        use CopyingType::{ByteStream, Datarams};
        match self {
            Endpoint::TcpConnectByIp(_) => ByteStream,
            Endpoint::TcpListen(_) => ByteStream,
            Endpoint::TcpListenFd(_) => ByteStream,
            Endpoint::TcpListenFdNamed(_) => ByteStream,
            Endpoint::TcpConnectByEarlyHostname { .. } => ByteStream,
            Endpoint::TcpConnectByLateHostname { hostname: _ } => ByteStream,
            Endpoint::WsUrl(_) => Datarams,
            Endpoint::WssUrl(_) => Datarams,
            Endpoint::Stdio => ByteStream,
            Endpoint::UdpConnect(_) => Datarams,
            Endpoint::UdpBind(_) => Datarams,
            Endpoint::UdpFd(_) => Datarams,
            Endpoint::UdpFdNamed(_) => Datarams,
            Endpoint::WsListen(_) => Datarams,
            Endpoint::UdpServer(_) => Datarams,
            Endpoint::UdpServerFd(_) => Datarams,
            Endpoint::UdpServerFdNamed(_) => Datarams,
            Endpoint::Exec(_) => ByteStream,
            Endpoint::Cmd(_) => ByteStream,
            Endpoint::DummyStream => ByteStream,
            Endpoint::DummyDatagrams => Datarams,
            Endpoint::Literal(_) => ByteStream,
            Endpoint::LiteralBase64(_) => ByteStream,
            Endpoint::UnixConnect(_) => ByteStream,
            Endpoint::UnixListen(_) => ByteStream,
            Endpoint::AbstractConnect(_) => ByteStream,
            Endpoint::AbstractListen(_) => ByteStream,
            Endpoint::UnixListenFd(_) => ByteStream,
            Endpoint::UnixListenFdNamed(_) => ByteStream,
            Endpoint::SeqpacketConnect(_) => Datarams,
            Endpoint::SeqpacketListen(_) => Datarams,
            Endpoint::AbstractSeqpacketConnect(_) => Datarams,
            Endpoint::AbstractSeqpacketListen(_) => Datarams,
            Endpoint::SeqpacketListenFd(_) => Datarams,
            Endpoint::SeqpacketListenFdNamed(_) => Datarams,
            Endpoint::MockStreamSocket(_) => ByteStream,
            Endpoint::RegistryStreamListen(_) => ByteStream,
            Endpoint::RegistryStreamConnect(_) => ByteStream,
            Endpoint::AsyncFd(_) => ByteStream,
            Endpoint::SimpleReuserEndpoint(..) => Datarams,
            Endpoint::ReadFile(..) => ByteStream,
            Endpoint::WriteFile(..) => ByteStream,
            Endpoint::AppendFile(..) => ByteStream,
            Endpoint::Random => ByteStream,
            Endpoint::Zero => ByteStream,
            Endpoint::WriteSplitoff { read, write } => {
                match (read.get_copying_type(), write.get_copying_type()) {
                    (ByteStream, ByteStream) => ByteStream,
                    (Datarams, Datarams) => Datarams,
                    _ => panic!("Incompatibe Socket types for WriteSplitoff"),
                }
            }
        }
    }
}

impl Overlay {
    fn get_copying_type(&self) -> Option<CopyingType> {
        use CopyingType::{ByteStream, Datarams};
        Some(match self {
            Overlay::WsUpgrade { .. } => ByteStream,
            Overlay::WsFramer { .. } => Datarams,
            Overlay::StreamChunks => Datarams,
            Overlay::LineChunks => Datarams,
            Overlay::TlsClient { .. } => ByteStream,
            Overlay::WsAccept {} => ByteStream,
            Overlay::Log { datagram_mode } => {
                if *datagram_mode {
                    Datarams
                } else {
                    ByteStream
                }
            }
            Overlay::WsClient => Datarams,
            Overlay::WsServer => Datarams,
            Overlay::ReadChunkLimiter => ByteStream,
            Overlay::WriteChunkLimiter => ByteStream,
            Overlay::WriteBuffer => ByteStream,
            Overlay::LengthPrefixedChunks => Datarams,
            Overlay::SimpleReuser => Datarams,
            Overlay::WriteSplitoff => return None,
        })
    }
}

impl SpecifierStack {
    /// Expected to emit multiple connections in parallel
    pub(super) fn is_multiconn(&self, opts: &WebsocatArgs) -> bool {
        let mut multiconn = match self.innermost {
            Endpoint::TcpConnectByEarlyHostname { .. } => false,
            Endpoint::TcpConnectByLateHostname { .. } => false,
            Endpoint::TcpConnectByIp(..) => false,
            Endpoint::TcpListen(..) => !opts.oneshot,
            Endpoint::TcpListenFd(..) => !opts.oneshot,
            Endpoint::TcpListenFdNamed(..) => !opts.oneshot,
            Endpoint::WsUrl(..) => false,
            Endpoint::WssUrl(..) => false,
            Endpoint::WsListen(..) => !opts.oneshot,
            Endpoint::Stdio => false,
            Endpoint::UdpConnect(..) => false,
            Endpoint::UdpBind(..) => false,
            Endpoint::UdpFd(_) => false,
            Endpoint::UdpFdNamed(_) => false,
            Endpoint::UdpServer(..) => !opts.oneshot,
            Endpoint::UdpServerFd(_) => !opts.oneshot,
            Endpoint::UdpServerFdNamed(_) => !opts.oneshot,
            Endpoint::Exec(..) => false,
            Endpoint::Cmd(..) => false,
            Endpoint::DummyStream => false,
            Endpoint::DummyDatagrams => false,
            Endpoint::Literal(_) => false,
            Endpoint::LiteralBase64(_) => false,
            Endpoint::UnixConnect(..) => false,
            Endpoint::UnixListen(..) => !opts.oneshot,
            Endpoint::AbstractConnect(..) => false,
            Endpoint::AbstractListen(..) => !opts.oneshot,
            Endpoint::UnixListenFd(_) => !opts.oneshot,
            Endpoint::UnixListenFdNamed(_) => !opts.oneshot,
            Endpoint::AsyncFd(_) => false,
            Endpoint::SeqpacketConnect(..) => false,
            Endpoint::SeqpacketListen(..) => !opts.oneshot,
            Endpoint::AbstractSeqpacketConnect(..) => false,
            Endpoint::AbstractSeqpacketListen(..) => !opts.oneshot,
            Endpoint::SeqpacketListenFd(..) => !opts.oneshot,
            Endpoint::SeqpacketListenFdNamed(..) => !opts.oneshot,
            Endpoint::MockStreamSocket(..) => false,
            Endpoint::RegistryStreamListen(..) => !opts.oneshot,
            Endpoint::RegistryStreamConnect(..) => false,
            Endpoint::SimpleReuserEndpoint(..) => false,
            Endpoint::ReadFile(..) => false,
            Endpoint::WriteFile(..) => false,
            Endpoint::AppendFile(..) => false,
            Endpoint::Random => false,
            Endpoint::Zero => false,
            Endpoint::WriteSplitoff { .. } => false,
        };

        for x in &self.overlays {
            match x {
                Overlay::WsUpgrade { .. } => {}
                Overlay::WsAccept { .. } => {}
                Overlay::WsFramer { .. } => {}
                Overlay::WsClient => {}
                Overlay::WsServer => {}
                Overlay::TlsClient { .. } => {}
                Overlay::StreamChunks => {}
                Overlay::LineChunks => {}
                Overlay::LengthPrefixedChunks => {}
                Overlay::Log { .. } => {}
                Overlay::ReadChunkLimiter => {}
                Overlay::WriteChunkLimiter => {}
                Overlay::WriteBuffer => {}
                Overlay::SimpleReuser => multiconn = false,
                Overlay::WriteSplitoff => multiconn = false,
            }
        }

        multiconn
    }

    /// Does not like reentrant usage
    fn prefers_being_single(&self, opts: &WebsocatArgs) -> bool {
        let mut singler = match self.innermost {
            Endpoint::TcpConnectByEarlyHostname { .. } => false,
            Endpoint::TcpConnectByLateHostname { .. } => false,
            Endpoint::TcpConnectByIp(..) => false,
            Endpoint::TcpListen(..) => false,
            Endpoint::TcpListenFd(..) => false,
            Endpoint::TcpListenFdNamed(..) => false,
            Endpoint::WsUrl(..) => false,
            Endpoint::WssUrl(..) => false,
            Endpoint::WsListen(..) => false,
            Endpoint::Stdio => true,
            Endpoint::UdpConnect(..) => false,
            Endpoint::UdpBind(..) => true,
            Endpoint::UdpFd(_) => true,
            Endpoint::UdpFdNamed(_) => true,
            Endpoint::UdpServer(..) => false,
            Endpoint::UdpServerFd(_) => false,
            Endpoint::UdpServerFdNamed(_) => false,
            Endpoint::Exec(..) => false,
            Endpoint::Cmd(..) => false,
            Endpoint::DummyStream => false,
            Endpoint::DummyDatagrams => false,
            Endpoint::Literal(_) => false,
            Endpoint::LiteralBase64(_) => false,
            Endpoint::UnixConnect(..) => false,
            Endpoint::UnixListen(..) => false,
            Endpoint::AbstractConnect(..) => false,
            Endpoint::AbstractListen(..) => false,
            Endpoint::UnixListenFd(_) => false,
            Endpoint::UnixListenFdNamed(_) => false,
            Endpoint::AsyncFd(_) => true,
            Endpoint::SeqpacketConnect(..) => false,
            Endpoint::SeqpacketListen(..) => false,
            Endpoint::AbstractSeqpacketConnect(..) => false,
            Endpoint::AbstractSeqpacketListen(..) => false,
            Endpoint::SeqpacketListenFd(..) => false,
            Endpoint::SeqpacketListenFdNamed(..) => false,
            Endpoint::MockStreamSocket(..) => false,
            Endpoint::RegistryStreamListen(..) => false,
            Endpoint::RegistryStreamConnect(..) => false,
            Endpoint::SimpleReuserEndpoint(..) => false,
            Endpoint::ReadFile(..) => false,
            Endpoint::WriteFile(..) => !opts.write_file_no_overwrite,
            Endpoint::AppendFile(..) => true,
            Endpoint::Random => false,
            Endpoint::Zero => false,
            Endpoint::WriteSplitoff {
                ref read,
                ref write,
            } => read.prefers_being_single(opts) || write.prefers_being_single(opts),
        };

        for x in &self.overlays {
            match x {
                Overlay::WsUpgrade { .. } => {}
                Overlay::WsAccept { .. } => {}
                Overlay::WsFramer { .. } => {}
                Overlay::WsClient => {}
                Overlay::WsServer => {}
                Overlay::TlsClient { .. } => {}
                Overlay::StreamChunks => {}
                Overlay::LineChunks => {}
                Overlay::LengthPrefixedChunks => {}
                Overlay::Log { .. } => {}
                Overlay::ReadChunkLimiter => {}
                Overlay::WriteChunkLimiter => {}
                Overlay::WriteBuffer => {}
                Overlay::SimpleReuser => singler = false,
                Overlay::WriteSplitoff => {}
            }
        }

        singler
    }
}
