use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Endpoint,
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print_tcp(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = vars.getnewvarname("tcp");
                printer.print_line(&format!(
                    "connect_tcp(#{{addr: {a}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
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
                printer.print_line(&format!(
                    "lookup_host({h}, |{addrs}| {{",
                    h = StrLit(hostname)
                ));
                printer.increase_indent();

                let varnam = vars.getnewvarname("tcp");
                printer.print_line(&format!("connect_tcp_race(#{{}}, {addrs}, |{varnam}| {{"));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpListen(addr) => {
                let varnam = vars.getnewvarname("tcp");
                let fromaddr = vars.getnewvarname("from");
                let listenparams = opts.listening_parameters();
                printer.print_line(&format!(
                    "listen_tcp(#{{{listenparams}, addr: {a}}}, |{varnam}, {fromaddr}| {{",
                    a = StrLit(addr),
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_tcp(&self, printer: &mut ScenarioPrinter, opts: &WebsocatArgs) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpConnectByEarlyHostname { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpListen(addr) => {
                printer.decrease_indent();
                printer.print_line("}, |listen_addr|{sequential([");
                printer.increase_indent();
                if opts.stdout_announce_listening_ports {
                    let ip = addr.ip();
                    let port = addr.port();
                    printer.print_line(&format!("print_stdout(\"LISTEN proto=tcp,ip={ip},port={port}\\n\")"));
                }
                printer.decrease_indent();
                printer.print_line("])})");
            }
            Endpoint::TcpConnectByLateHostname { hostname: _ } => {
                printer.decrease_indent();
                printer.print_line("})");

                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
