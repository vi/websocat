use std::net::{IpAddr, SocketAddr};

use http::{uri::Authority, Uri};
use tracing::{debug, warn};

use crate::cli::WebsocatArgs;

use super::{
    types::{
        Endpoint, EndpointDiscriminants, Overlay, OverlayDiscriminants, PreparatoryAction,
        SocketType, SpecifierStack, WebsocatInvocation, WebsocatInvocationStacks,
    },
    utils::IdentifierGenerator,
};

impl WebsocatInvocation {
    pub fn patches(&mut self, vars: &mut IdentifierGenerator) -> anyhow::Result<()> {
        self.stacks
            .apply_to_all(|x| x.maybe_patch_existing_ws_request(&self.opts))?;
        self.stacks
            .apply_to_all(|x| x.maybe_splitup_client_ws_endpoint())?;
        self.stacks
            .apply_to_all(|x| x.maybe_splitup_ws_c_overlay(&self.opts))?;
        self.stacks
            .apply_to_all(|x| x.maybe_splitup_ws_u_overlay(&self.opts))?;
        self.stacks
            .apply_to_all(|x| x.maybe_splitup_server_ws_endpoint())?;
        if !self.opts.late_resolve {
            self.stacks
                .apply_to_all(|x| x.maybe_early_resolve(&mut self.beginning, vars))?;
        }
        self.maybe_fill_in_tls_details(vars)?;
        if self.opts.log_traffic
            && !self.stacks.right.insert_log_overlay()
            && !self.stacks.left.insert_log_overlay()
        {
            warn!("Failed to automaticelly insert log: overlay");
        }
        self.stacks.apply_to_all(|x| x.fill_in_log_overlay_type())?;

        self.stacks
            .left
            .maybe_process_writesplitoff(&mut self.stacks.write_splitoff, &self.opts)?;
        self.stacks
            .right
            .maybe_process_writesplitoff(&mut self.stacks.write_splitoff, &self.opts)?;

        self.maybe_insert_chunker();

        if !self.opts.less_fixups {
            self.maybe_insert_reuser();
        }

        self.stacks
            .apply_to_all(|x| x.maybe_process_reuser(vars, &mut self.beginning))?;

        self.stacks
            .apply_to_all(|x| x.check_required_socket_types(&self.opts))?;
        Ok(())
    }
}
impl WebsocatInvocationStacks {
    fn apply_to_all(
        &mut self,
        mut f: impl FnMut(&mut SpecifierStack) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        f(&mut self.left)?;
        f(&mut self.right)?;
        if let Some(ref mut splt) = self.write_splitoff {
            f(splt)?;
        }
        Ok(())
    }
}
impl WebsocatInvocation {
    fn maybe_insert_chunker(&mut self) {
        if self.opts.exec_dup2.is_some() {
            // dup2 mode is speial, it ignores any overlays and directly forwards
            // file desciptor to process. messages vs bytestreams distinction does not matter in this mode.
            return;
        }

        // use Datagrams copying type (and auto-insert appropriate overlays) if at least one of things expects datagrams.

        let mut working_copying_type = self.stacks.left.provides_socket_type();
        if self.stacks.right.provides_socket_type().is_dgrms() {
            working_copying_type = SocketType::Datarams;
        }

        /*if let Some(ref spl) = self.write_splitoff {
            if spl.get_copying_type().is_dgrms() {
                working_copying_type = CopyingType::Datarams;
            }
        }*/

        if working_copying_type == SocketType::ByteStream {
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

        if self.stacks.left.provides_socket_type().is_bstrm() {
            self.stacks.left.overlays.push(overlay_to_insert());
        }
        if self.stacks.right.provides_socket_type().is_bstrm() {
            self.stacks.right.overlays.push(overlay_to_insert());
        }

        /*if let Some(ref mut spl) = self.write_splitoff {
            if spl.get_copying_type().is_bstrm() {
                spl.overlays.push(overlay_to_insert());
            }
        }*/
        assert_eq!(
            self.stacks.left.provides_socket_type(),
            self.stacks.right.provides_socket_type()
        );
    }

    fn maybe_insert_reuser(&mut self) {
        if self.session_socket_type() == SocketType::Datarams
            && self.stacks.left.is_multiconn(&self.opts)
            && self.stacks.right.prefers_being_single(&self.opts)
        {
            self.stacks.right.overlays.push(Overlay::SimpleReuser);
        }
    }

    fn maybe_fill_in_tls_details(&mut self, vars: &mut IdentifierGenerator) -> anyhow::Result<()> {
        if let Some(ref d) = self.opts.tls_domain {
            let mut patch_occurred = false;
            for x in self
                .stacks
                .left
                .overlays
                .iter_mut()
                .chain(self.stacks.right.overlays.iter_mut())
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
        for x in self
            .stacks
            .left
            .overlays
            .iter()
            .chain(self.stacks.right.overlays.iter())
        {
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
            .stacks
            .left
            .overlays
            .iter_mut()
            .chain(self.stacks.right.overlays.iter_mut())
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
    ) -> anyhow::Result<()> {
        if let Endpoint::TcpConnectByLateHostname { hostname } = &self.innermost {
            let varname_for_addrs = vars.getnewvarname("addrs");
            beginning.push(PreparatoryAction::ResolveHostname {
                hostname: hostname.clone(),
                varname_for_addrs: varname_for_addrs.clone(),
            });
            self.innermost = Endpoint::TcpConnectByEarlyHostname { varname_for_addrs };
        }
        Ok(())
    }

    fn fill_in_log_overlay_type(&mut self) -> anyhow::Result<()> {
        let mut typ = self.innermost.provides_socket_type();
        for ovl in &mut self.overlays {
            match ovl {
                Overlay::Log { datagram_mode } => {
                    *datagram_mode = typ == SocketType::Datarams;
                }
                x => {
                    if let Some(nct) = x.provides_socket_type() {
                        typ = nct
                    }
                }
            }
        }
        Ok(())
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
        opts: &WebsocatArgs,
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

        let mut read = switcheroo;
        let mut write = Box::new(write_splitoff_stack);

        let overlay_to_insert = || {
            if opts.binary {
                Overlay::StreamChunks
            } else {
                Overlay::LineChunks
            }
        };

        match (read.provides_socket_type(), write.provides_socket_type()) {
            (SocketType::ByteStream, SocketType::ByteStream) => {}
            (SocketType::ByteStream, SocketType::Datarams) => {
                read.overlays.push(overlay_to_insert())
            }
            (SocketType::Datarams, SocketType::ByteStream) => {
                write.overlays.push(overlay_to_insert())
            }
            (SocketType::Datarams, SocketType::Datarams) => {}
        }

        self.innermost = Endpoint::WriteSplitoff { read, write };

        // continue processing deeper just to handle possible duplicate errors
        self.maybe_process_writesplitoff(&mut None, opts)?;

        Ok(())
    }

    fn check_required_socket_types(&self, _opts: &WebsocatArgs) -> anyhow::Result<()> {
        enum SocketProvider {
            Endpoint(EndpointDiscriminants),
            Overlay(OverlayDiscriminants),
        }
        impl std::fmt::Debug for SocketProvider {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Endpoint(x) => x.fmt(f),
                    Self::Overlay(x) => x.fmt(f),
                }
            }
        }

        let mut origin = SocketProvider::Endpoint((&self.innermost).into());
        let mut curtyp = self.innermost.provides_socket_type();

        for ovl in &self.overlays {
            if let Some(rq) = ovl.requires_socket_type() {
                if rq != curtyp {
                    let ovlt: OverlayDiscriminants = ovl.into();
                    anyhow::bail!("{ovlt:?} requires {rq:?} socket type, but {origin:?} provides a {curtyp:?} socket");
                }
            }
            if let Some(pr) = ovl.provides_socket_type() {
                curtyp = pr;
                origin = SocketProvider::Overlay(ovl.into())
            }
        }

        Ok(())
    }
}
