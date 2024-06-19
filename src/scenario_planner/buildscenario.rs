use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::ScenarioPrinter,
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

        left = self.left.innermost.begin_print(&mut printer, vars)?;

        for ovl in &self.left.overlays {
            left = ovl.begin_print(&mut printer, &left, vars)?;
        }

        right = self.right.innermost.begin_print(&mut printer, vars)?;

        for ovl in &self.right.overlays {
            right = ovl.begin_print(&mut printer, &right, vars)?;
        }

        match self.get_copying_type() {
            CopyingType::ByteStream => {
                printer.print_line(&format!("exchange_bytes(#{{}}, {left}, {right})"));
            }
            CopyingType::Datarams => {
                printer.print_line(&format!("exchange_packets(#{{}}, {left}, {right})"));
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
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = vars.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp(#{{addr: \"{addr}\"}}, |{varnam}| {{"));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpConnectByEarlyHostname { varname_for_addrs } => {
                let varnam = vars.getnewvarname("tcp");
                printer.print_line(&format!(
                    "connect_tcp_race(#{{}}, {varname_for_addrs}, |{varnam}| {{"
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpConnectByLateHostname { hostname } => {
                let addrs = vars.getnewvarname("addrs");
                printer.print_line(&format!("lookup_host(\"{hostname}\", |{addrs}| {{"));
                printer.increase_indent();

                let varnam = vars.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp_race(#{{}}, {addrs}, |{varnam}| {{"));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpListen(addr) => {
                let varnam = vars.getnewvarname("tcp");
                let fromaddr = vars.getnewvarname("from");
                printer.print_line(&format!(
                    "listen_tcp(#{{autospawn: true, addr: \"{addr}\"}}, |{varnam}, {fromaddr}| {{"
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::WsUrl(..) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {
                let varnam = vars.getnewvarname("stdio");
                printer.print_line(&format!("let {varnam} = create_stdio();"));
                Ok(varnam)
            }
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpConnectByEarlyHostname { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpListen(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::WsUrl(_) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            Endpoint::WssUrl(_) => todo!(),
            Endpoint::Stdio => {}
            Endpoint::UdpConnect(_) => todo!(),
            Endpoint::UdpBind(_) => todo!(),
            Endpoint::TcpConnectByLateHostname { hostname: _ } => {
                printer.decrease_indent();
                printer.print_line("})");

                printer.decrease_indent();
                printer.print_line("})");
            }
        }
    }
}

impl Overlay {
    fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        inner_var: &str,
        vars: &mut IdentifierGenerator,
    ) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade { uri, host } => {
                let wsframes = vars.getnewvarname("wsframes");

                printer.print_line(&format!("ws_upgrade({inner_var}, #{{host: \"{host}\", url: \"{uri}\"}}, |{wsframes}| {{"));
                printer.increase_indent();

                Ok(wsframes)
            }
            Overlay::WsFramer { client_mode } => {
                let ws = vars.getnewvarname("ws");
                printer.print_line(&format!(
                    "let {ws} = ws_wrap(#{{client: {client_mode}}}, {inner_var});"
                ));

                Ok(ws)
            }
            Overlay::StreamChunks => {
                let varnam = vars.getnewvarname("chunks");
                printer.print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
            Overlay::TlsClient {
                domain,
                varname_for_connector,
            } => {
                assert!(!varname_for_connector.is_empty());
                let outer_var = vars.getnewvarname("tls");

                printer.print_line(&format!("tls_client(#{{domain: \"{domain}\"}}, {varname_for_connector}, {inner_var}, |{outer_var}| {{"));
                printer.increase_indent();

                Ok(outer_var)
            }
        }
    }
    fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::WsFramer { .. } => (),
            Overlay::StreamChunks => (),
            Overlay::TlsClient { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
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
                    "lookup_host(\"{hostname}\", |{varname_for_addrs}| {{"
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
