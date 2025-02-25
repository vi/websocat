use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Endpoint,
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print(
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
            Endpoint::MockStreamSocket(s) => {
                let varnam = vars.getnewvarname("mock");
                printer.print_line(&format!(
                    "let {varnam} = mock_stream_socket({});",
                    StrLit(s)
                ));
                Ok(varnam)
            }
            Endpoint::RegistryStreamListen(addr) => {
                let listenparams = opts.listening_parameters();
                let varnam = vars.getnewvarname("reg");
                printer.print_line(&format!(
                    "listen_registry_stream(#{{{listenparams}, addr: {a}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::RegistryStreamConnect(addr) => {
                let mbs = opts.registry_connect_bufsize;
                let varnam = vars.getnewvarname("reg");
                printer.print_line(&format!(
                    "connect_registry_stream(#{{addr: {a}, max_buf_size: {mbs}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
                printer.increase_indent();
                Ok(varnam)
            }
        }
    }

    pub(super) fn end_print(&self, printer: &mut ScenarioPrinter, opts: &WebsocatArgs) {
        match self {
            Endpoint::TcpConnectByIp(..)
            | Endpoint::TcpConnectByEarlyHostname { .. }
            | Endpoint::TcpListen { .. }
            | Endpoint::TcpConnectByLateHostname { .. } => self.end_print_tcp(printer, opts),
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
            Endpoint::MockStreamSocket(_) => {}
            Endpoint::RegistryStreamListen(_) | Endpoint::RegistryStreamConnect(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
        }
    }
}
