use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::{CopyingType, Endpoint, Overlay, PreparatoryAction, WebsocatInvocation},
    utils::IdentifierGenerator,
};

impl WebsocatInvocation {
    pub fn build_scenario(self, vars: &mut IdentifierGenerator) -> anyhow::Result<String> {
        let mut printer = ScenarioPrinter::new();

        let mut left: String;
        let mut right: String;

        for prepare_action in &self.beginning {
            prepare_action.begin_print(&mut printer, &self.opts, vars)?;
        }

        left = self
            .left
            .innermost
            .begin_print(&mut printer, vars, &self.opts)?;

        for ovl in &self.left.overlays {
            left = ovl.begin_print(&mut printer, &left, vars, &self.opts)?;
        }

        right = self
            .right
            .innermost
            .begin_print(&mut printer, vars, &self.opts)?;

        for ovl in &self.right.overlays {
            right = ovl.begin_print(&mut printer, &right, vars, &self.opts)?;
        }

        if self.opts.exit_on_hangup {
            printer.print_line(&format!(
                "try {{ handle_hangup(take_hangup_part({left}), || {{  sleep_ms(50); exit_process(0); }} ); }} catch {{}}")
            );
            printer.print_line(&format!(
                "try {{ handle_hangup(take_hangup_part({right}), || {{  sleep_ms(50); exit_process(0); }} ); }} catch {{}}")
            );
        }

        let mut opts = String::with_capacity(64);
        if self.opts.unidirectional {
            opts.push_str("unidirectional: true,");
        }
        if self.opts.unidirectional_reverse {
            opts.push_str("unidirectional_reverse: true,");
        }
        if self.opts.exit_on_eof {
            opts.push_str("exit_on_eof: true,");
        }
        if self.opts.unidirectional_late_drop {
            opts.push_str("unidirectional_late_drop: true,");
        }
        if let Some(ref bs) = self.opts.buffer_size {
            opts.push_str(&format!("buffer_size_forward: {bs},"));
            opts.push_str(&format!("buffer_size_reverse: {bs},"));
        }
        match self.get_copying_type() {
            CopyingType::ByteStream => {
                printer.print_line(&format!("exchange_bytes(#{{{opts}}}, {left}, {right})"));
            }
            CopyingType::Datarams => {
                printer.print_line(&format!("exchange_packets(#{{{opts}}}, {left}, {right})"));
            }
        }

        for ovl in self.right.overlays.iter().rev() {
            ovl.end_print(&mut printer);
        }

        self.right.innermost.end_print(&mut printer);

        for ovl in self.left.overlays.iter().rev() {
            ovl.end_print(&mut printer);
        }

        self.left.innermost.end_print(&mut printer);

        for prepare_action in self.beginning.iter().rev() {
            prepare_action.end_print(&mut printer);
        }

        Ok(printer.into_result())
    }
}

impl Endpoint {
    fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(..)
            | Endpoint::TcpConnectByEarlyHostname { .. }
            | Endpoint::TcpListen { .. }
            | Endpoint::TcpConnectByLateHostname { .. } => {
                self.begin_print_tcp(printer, vars, opts)
            }
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                self.begin_print_ws(printer, vars, opts)
            }
            Endpoint::Stdio => {
                let varnam = vars.getnewvarname("stdio");
                printer.print_line(&format!("let {varnam} = create_stdio();"));
                Ok(varnam)
            }
            Endpoint::UdpConnect(..) | Endpoint::UdpBind(..) | Endpoint::UdpServer(..) => {
                self.begin_print_udp(printer, vars, opts)
            }
            Endpoint::Exec(..) | Endpoint::Cmd(..) => self.begin_print_exec(printer, vars, opts),
            Endpoint::DummyStream => {
                let varnam = vars.getnewvarname("dummy");
                printer.print_line(&format!("let {varnam} = dummy_stream_socket();"));
                if opts.dummy_hangup {
                    printer.print_line(&format!(
                        "put_hangup_part({varnam}, pre_triggered_hangup_handle());"
                    ));
                }
                Ok(varnam)
            }
            Endpoint::DummyDatagrams => {
                let varnam = vars.getnewvarname("dummy");
                printer.print_line(&format!("let {varnam} = dummy_datagram_socket();"));
                if opts.dummy_hangup {
                    printer.print_line(&format!(
                        "put_hangup_part({varnam}, pre_triggered_hangup_handle());"
                    ));
                }
                Ok(varnam)
            }
            Endpoint::Literal(s) => {
                let varnam = vars.getnewvarname("lit");
                printer.print_line(&format!("let {varnam} = literal_socket({});", StrLit(s)));
                Ok(varnam)
            }
            Endpoint::LiteralBase64(s) => {
                let varnam = vars.getnewvarname("lit");
                printer.print_line(&format!(
                    "let {varnam} = literal_socket_base64({});",
                    StrLit(s)
                ));
                Ok(varnam)
            }
            Endpoint::UnixConnect(..)
            | Endpoint::UnixListen(..)
            | Endpoint::AbstractConnect(_)
            | Endpoint::AbstractListen(_)
            | Endpoint::SeqpacketConnect(_)
            | Endpoint::SeqpacketListen(_)
            | Endpoint::AbstractSeqpacketConnect(_)
            | Endpoint::AbstractSeqpacketListen(_) => self.begin_print_unix(printer, vars, opts),
        }
    }

    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::TcpConnectByIp(..)
            | Endpoint::TcpConnectByEarlyHostname { .. }
            | Endpoint::TcpListen { .. }
            | Endpoint::TcpConnectByLateHostname { .. } => self.end_print_tcp(printer),
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                self.end_print_ws(printer)
            }
            Endpoint::Stdio => {}
            Endpoint::UdpConnect(_) | Endpoint::UdpBind(_) | Endpoint::UdpServer(_) => {
                self.end_print_udp(printer)
            }
            Endpoint::Exec(_) | Endpoint::Cmd(_) => self.end_print_exec(printer),
            Endpoint::DummyStream => {}
            Endpoint::DummyDatagrams => {}
            Endpoint::Literal(_) => {}
            Endpoint::LiteralBase64(_) => {}
            Endpoint::UnixConnect(_)
            | Endpoint::UnixListen(_)
            | Endpoint::AbstractConnect(_)
            | Endpoint::AbstractListen(_)
            | Endpoint::SeqpacketConnect(_)
            | Endpoint::SeqpacketListen(_)
            | Endpoint::AbstractSeqpacketConnect(_)
            | Endpoint::AbstractSeqpacketListen(_) => self.end_print_unix(printer),
        }
    }
}

impl Overlay {
    fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        inner_var: &str,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.begin_print_ws(printer, inner_var, vars, opts),
            Overlay::StreamChunks => {
                let varnam = vars.getnewvarname("chunks");
                printer.print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
            Overlay::LineChunks => {
                let varnam = vars.getnewvarname("chunks");
                let mut oo = String::new();
                if let Some(ref x) = opts.separator {
                    oo.push_str(&format!("separator: {x},"));
                }
                if let Some(ref x) = opts.separator_n {
                    oo.push_str(&format!("separator_n: {x},"));
                }
                if !opts.separator_inhibit_substitution {
                    oo.push_str(&format!("substitute: 32,"));
                }
                printer.print_line(&format!(
                    "let {varnam} = line_chunks(#{{{oo}}}, {inner_var});"
                ));
                Ok(varnam)
            }
            Overlay::TlsClient {
                domain,
                varname_for_connector,
            } => {
                assert!(!varname_for_connector.is_empty());
                let outer_var = vars.getnewvarname("tls");

                printer.print_line(&format!("tls_client(#{{domain: {dom}}}, {varname_for_connector}, {inner_var}, |{outer_var}| {{", dom=StrLit(domain)));
                printer.increase_indent();

                Ok(outer_var)
            }
            Overlay::Log { datagram_mode } => {
                let varnam = vars.getnewvarname("log");

                let funcname = if *datagram_mode {
                    "datagram_logger"
                } else {
                    "stream_logger"
                };

                let maybe_loghex = if opts.log_hex { "hex: true," } else { "" };

                let maybe_log_omit_content = if opts.log_omit_content {
                    "omit_content: true,"
                } else {
                    ""
                };

                let maybe_log_verbose = if opts.log_verbose {
                    "verbose: true,"
                } else {
                    ""
                };

                printer.print_line(&format!("let {varnam} = {funcname}(#{{{maybe_loghex}{maybe_log_omit_content}{maybe_log_verbose}}}, {inner_var});"));
                Ok(varnam)
            }
            Overlay::ReadChunkLimiter => {
                let n = opts.read_buffer_limit.unwrap_or(1);
                printer.print_line(&format!("put_read_part({inner_var}, read_chunk_limiter(take_read_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteChunkLimiter => {
                let n = opts.write_buffer_limit.unwrap_or(1);
                printer.print_line(&format!("put_write_part({inner_var}, write_chunk_limiter(take_write_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteBuffer => {
                printer.print_line(&format!("put_write_part({inner_var}, write_buffer(take_write_part({inner_var}), 8192));"));
                Ok(inner_var.to_owned())
            }
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.end_print_ws(printer),
            Overlay::StreamChunks => (),
            Overlay::LineChunks => (),
            Overlay::TlsClient { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::Log { .. } => (),
            Overlay::ReadChunkLimiter => (),
            Overlay::WriteChunkLimiter => (),
            Overlay::WriteBuffer => (),
        }
    }
}

impl PreparatoryAction {
    fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        opts: &WebsocatArgs,
        _vars: &mut IdentifierGenerator,
    ) -> anyhow::Result<()> {
        match self {
            PreparatoryAction::ResolveHostname {
                hostname,
                varname_for_addrs,
            } => {
                printer.print_line(&format!(
                    "lookup_host({hn}, |{varname_for_addrs}| {{",
                    hn = StrLit(hostname),
                ));
                printer.increase_indent();
            }
            PreparatoryAction::CreateTlsConnector {
                varname_for_connector,
            } => {
                if opts.insecure {
                    printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{danger_accept_invalid_certs: true, danger_accept_invalid_hostnames: true}});"
                    ));
                } else {
                    printer.print_line(&format!(
                        "let {varname_for_connector} = tls_client_connector(#{{}});"
                    ));
                }
            }
        }
        Ok(())
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            PreparatoryAction::ResolveHostname { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            PreparatoryAction::CreateTlsConnector { .. } => (),
        }
    }
}
