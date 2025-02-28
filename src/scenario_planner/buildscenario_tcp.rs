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
            Endpoint::TcpListen(..)
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..) => {
                let varnam = vars.getnewvarname("tcp");
                let fromaddr = vars.getnewvarname("from");
                let listenparams = opts.listening_parameters();

                let addrpart = match self {
                    Endpoint::TcpListen(addr) => {
                        format!("addr: {a}", a = StrLit(addr),)
                    }
                    Endpoint::TcpListenFd(fd) => {
                        format!("fd: {fd}",)
                    }
                    Endpoint::TcpListenFdNamed(fdname) => {
                        format!("named_fd: {a}", a = StrLit(fdname),)
                    }
                    _ => unreachable!(),
                };
                printer.print_line(&format!(
                    "listen_tcp(#{{{listenparams}, {addrpart}}}, |listen_addr|{{sequential([",
                ));
                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line("print_stdout(\"LISTEN proto=tcp,ip=\"+listen_addr.get_ip()+\",port=\"+str(listen_addr.get_port())+\"\\n\"),");
                }
                if let Some(ref x) = opts.exec_after_listen {
                    if opts.exec_after_listen_append_port {
                        printer.print_line(&format!(
                            "system({} + \" \" + str(listen_addr.get_port())),",
                            StrLit(x)
                        ));
                    } else {
                        printer.print_line(&format!("system({}),", StrLit(x)));
                    }
                }

                printer.decrease_indent();
                printer.print_line(&format!("])}},  |{varnam}, {fromaddr}| {{",));
                printer.increase_indent();

                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_tcp(&self, printer: &mut ScenarioPrinter, _opts: &WebsocatArgs) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpConnectByEarlyHostname { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::TcpListen(..)
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..) => {
                printer.decrease_indent();
                printer.print_line("})");
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
