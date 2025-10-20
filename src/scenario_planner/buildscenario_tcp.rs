use super::{
    scenarioprinter::StrLit,
    types::{Endpoint, ScenarioPrintingEnvironment},
};

fn tcp_common_bind_options(o: &mut String, env: &ScenarioPrintingEnvironment<'_>) {
    if let Some(v) = env.opts.reuseaddr {
        o.push_str(&format!("reuseaddr: {},", v));
    }
    if env.opts.reuseport {
        o.push_str(&format!("reuseport: true,"));
    }
    if let Some(ref v) = env.opts.bind_to_device {
        o.push_str(&format!("bind_device: {},", StrLit(v)));
    }
}

impl Endpoint {

    pub(super) fn begin_print_tcp(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(addr) => {
                let varnam = env.vars.getnewvarname("tcp");

                let mut o = String::with_capacity(32);
                o.push_str("addr: ");
                o.push_str(&format!("{},", StrLit(addr)));

                if let Some(bbc) = env.opts.bind_before_connect {
                    o.push_str(&format!("bind: {},", StrLit(bbc)));
                }
                tcp_common_bind_options(&mut o, env);

                env.printer
                    .print_line(&format!("connect_tcp(#{{{o}}}, |{varnam}| {{"));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpConnectByEarlyHostname { varname_for_addrs } => {

                let mut o = String::with_capacity(0);

                if let Some(bbc) = env.opts.bind_before_connect {
                    o.push_str(&format!("bind: {},", StrLit(bbc)));
                }
                tcp_common_bind_options(&mut o, env);

                let varnam = env.vars.getnewvarname("tcp");
                env.printer.print_line(&format!(
                    "connect_tcp_race(#{{{o}}}, {varname_for_addrs}, |{varnam}| {{"
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
                let mut o = String::with_capacity(0);
                if let Some(bbc) = env.opts.bind_before_connect {
                    o.push_str(&format!("bind: {},", StrLit(bbc)));
                }
                tcp_common_bind_options(&mut o, env);

                env.printer
                    .print_line(&format!("connect_tcp_race(#{{{o}}}, {addrs}, |{varnam}| {{"));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::TcpListen(..)
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..) => {
                let varnam = env.vars.getnewvarname("tcp");
                let fromaddr = env.vars.getnewvarname("from");
                let listenparams = env.opts.listening_parameters(env.position);

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

                let mut o = String::with_capacity(0);
                if matches!(self, Endpoint::TcpListen(..)) {
                    tcp_common_bind_options(&mut o, env);
                    if let Some(v) = env.opts.listen_backlog {
                        o.push_str(&format!("backlog: {},", v));
                    }
                }

                env.printer.print_line(&format!(
                    "listen_tcp(#{{{listenparams}, {addrpart}, {o}}}, |listen_addr|{{sequential([",
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
