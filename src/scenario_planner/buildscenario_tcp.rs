use super::{
    scenarioprinter::StrLit,
    types::{Endpoint, ScenarioPrintingEnvironment},
};

impl Endpoint {
    pub(super) fn begin_print_tcp(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = env.vars.getnewvarname("tcp");
                env.printer.print_line(&format!(
                    "connect_tcp(#{{addr: {a}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpConnectByEarlyHostname { varname_for_addrs } => {
                let varnam = env.vars.getnewvarname("tcp");
                env.printer.print_line(&format!(
                    "connect_tcp_race(#{{}}, {varname_for_addrs}, |{varnam}| {{"
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpConnectByLateHostname { hostname } => {
                let addrs = env.vars.getnewvarname("addrs");
                env.printer.print_line(&format!(
                    "lookup_host({h}, |{addrs}| {{",
                    h = StrLit(hostname)
                ));
                env.printer.increase_indent();

                let varnam = env.vars.getnewvarname("tcp");
                env.printer
                    .print_line(&format!("connect_tcp_race(#{{}}, {addrs}, |{varnam}| {{"));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpListen(..)
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..) => {
                let varnam = env.vars.getnewvarname("tcp");
                let fromaddr = env.vars.getnewvarname("from");
                let listenparams = env.opts.listening_parameters();

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
                env.printer.print_line(&format!(
                    "listen_tcp(#{{{listenparams}, {addrpart}}}, |listen_addr|{{sequential([",
                ));
                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer.print_line("print_stdout(\"LISTEN proto=tcp,ip=\"+listen_addr.get_ip()+\",port=\"+str(listen_addr.get_port())+\"\\n\"),");
                }
                if let Some(ref x) = env.opts.exec_after_listen {
                    if env.opts.exec_after_listen_append_port {
                        env.printer.print_line(&format!(
                            "system({} + \" \" + str(listen_addr.get_port())),",
                            StrLit(x)
                        ));
                    } else {
                        env.printer.print_line(&format!("system({}),", StrLit(x)));
                    }
                }

                env.printer.decrease_indent();
                env.printer
                    .print_line(&format!("])}},  |{varnam}, {fromaddr}| {{",));
                env.printer.increase_indent();

                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_tcp(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Endpoint::TcpConnectByIp(_) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::TcpConnectByEarlyHostname { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::TcpListen(..)
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::TcpConnectByLateHostname { hostname: _ } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");

                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
