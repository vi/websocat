use crate::scenario_executor::utils1::{ToNeutralAddress, NEUTRAL_SOCKADDR6};

use super::{
    scenarioprinter::StrLit,
    types::{Endpoint, ScenarioPrintingEnvironment},
};


fn udp_common_bind_options(o: &mut String, env: &ScenarioPrintingEnvironment<'_>) {
    super::buildscenario_tcp::tcp_common_bind_options(o, env);
}

fn udp_common_options(o: &mut String, env: &ScenarioPrintingEnvironment<'_>) {
    if let Some(v) = env.opts.socket_tclass_v6 {
        o.push_str(&format!("tclass_v6: {v},"));
    }
    if let Some(v) = env.opts.socket_tos_v4 {
        o.push_str(&format!("tos_v4: {v},"));
    }
    if let Some(v) = env.opts.socket_ttl {
        o.push_str(&format!("ttl: {v},"));
    }
    if let Some(v) = env.opts.socket_cpu_affinity {
        o.push_str(&format!("cpu_affinity: {v},"));
    }
    if let Some(v) = env.opts.socket_priority {
        o.push_str(&format!("priority: {v},"));
    }
    if let Some(v) = env.opts.socket_recv_buffer_size {
        o.push_str(&format!("recv_buffer_size: {v},"));
    }
    if let Some(v) = env.opts.socket_send_buffer_size {
        o.push_str(&format!("send_buffer_size: {v},"));
    }
    if let Some(v) = env.opts.socket_mark {
        o.push_str(&format!("mark: {v},"));
    }
}


impl Endpoint {
    pub(super) fn begin_print_udp(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::UdpConnect(a) => {
                let varnam = env.vars.getnewvarname("udp");
                let mut o = String::with_capacity(64);
                if env.opts.text {
                    o.push_str("tag_as_text: true,");
                }
                udp_common_bind_options(&mut o, env);
                udp_common_options(&mut o, env);
                env.printer.print_line(&format!(
                    "let {varnam} = udp_socket(#{{{o} addr: \"{a}\", max_send_datagram_size: {}}});",
                    env.opts.udp_max_send_datagram_size
                ));
                Ok(varnam)
            }
            Endpoint::UdpBind(_) | Endpoint::UdpFd(_) | Endpoint::UdpFdNamed(_) => {
                let varnam = env.vars.getnewvarname("udp");

                let mut udp_bind_redirect_to_last_seen_address =
                    env.opts.udp_bind_redirect_to_last_seen_address;
                let mut udp_bind_connect_to_first_seen_address =
                    env.opts.udp_bind_connect_to_first_seen_address;

                if env.opts.udp_bind_restrict_to_one_address
                    && env.opts.udp_bind_target_addr.is_none()
                {
                    anyhow::bail!("It is meaningless to --udp-bind-restrict-to-one-address without also specifying --udp-bind-target-addr")
                }
                if env.opts.udp_bind_restrict_to_one_address
                    && (env.opts.udp_bind_connect_to_first_seen_address
                        || env.opts.udp_bind_redirect_to_last_seen_address)
                {
                    anyhow::bail!("It is meaningless to use --udp-bind-restrict-to-one-address with another option to react at new incoming addresses")
                }

                if env.opts.udp_bind_target_addr.is_none() {
                    udp_bind_connect_to_first_seen_address = true;
                }
                if udp_bind_connect_to_first_seen_address {
                    udp_bind_redirect_to_last_seen_address = true;
                }

                let mut neutral = NEUTRAL_SOCKADDR6;

                let mut o = String::with_capacity(64);

                match self {
                    Endpoint::UdpBind(a) => {
                        o.push_str(&format!("bind: \"{a}\","));
                        neutral = a.to_neutral_address();
                    }
                    Endpoint::UdpFd(fd) => {
                        o.push_str(&format!("fd: {fd},"));
                    }
                    Endpoint::UdpFdNamed(fdname) => {
                        o.push_str(&format!("named_fd: {},", StrLit(fdname)));
                    }
                    _ => unreachable!(),
                }

                let toaddr = env.opts.udp_bind_target_addr.unwrap_or(neutral);
                o.push_str(&format!("addr: \"{toaddr}\","));
                o.push_str("sendto_mode: true,");

                if !env.opts.udp_bind_restrict_to_one_address {
                    o.push_str("allow_other_addresses: true,");
                }
                if udp_bind_redirect_to_last_seen_address {
                    o.push_str("redirect_to_last_seen_address: true,");
                }
                if udp_bind_connect_to_first_seen_address {
                    o.push_str("connect_to_first_seen_address: true,");
                }
                if env.opts.udp_bind_inhibit_send_errors {
                    o.push_str("inhibit_send_errors: true,");
                }
                o.push_str(&format!(
                    "max_send_datagram_size: {},",
                    env.opts.udp_max_send_datagram_size
                ));

                if env.opts.text {
                    o.push_str("tag_as_text: true,");
                }
                udp_common_bind_options(&mut o, env);
                udp_common_options(&mut o, env);

                env.printer
                    .print_line(&format!("let {varnam} = udp_socket(#{{{o}}});"));

                Ok(varnam)
            }
            Endpoint::UdpServer(_) | Endpoint::UdpServerFd(_) | Endpoint::UdpServerFdNamed(_) => {
                let varnam = env.vars.getnewvarname("udp");
                let fromaddr = env.vars.getnewvarname("from");

                let mut o = String::with_capacity(64);

                match self {
                    Endpoint::UdpServer(a) => {
                        o.push_str(&format!("bind: \"{a}\","));
                    }
                    Endpoint::UdpServerFd(fd) => {
                        o.push_str(&format!("fd: {fd},"));
                    }
                    Endpoint::UdpServerFdNamed(fdname) => {
                        o.push_str(&format!("named_fd: {},", StrLit(fdname)));
                    }
                    _ => unreachable!(),
                }

                if env.opts.udp_bind_inhibit_send_errors {
                    o.push_str("inhibit_send_errors: true,");
                }
                if env.opts.text {
                    o.push_str("tag_as_text: true,");
                }
                if env.opts.udp_server_backpressure {
                    o.push_str("backpressure: true,");
                }
                if let Some(x) = env.opts.udp_server_timeout_ms {
                    o.push_str(&format!("timeout_ms: {x},"));
                }
                if let Some(x) = env.opts.udp_server_max_clients {
                    o.push_str(&format!("max_clients: {x},"));
                }
                if let Some(x) = env.opts.udp_server_buffer_size {
                    o.push_str(&format!("buffer_size: {x},"));
                }
                if let Some(x) = env.opts.udp_server_qlen {
                    o.push_str(&format!("qlen: {x},"));
                }
                o.push_str(&format!(
                    "max_send_datagram_size: {},",
                    env.opts.udp_max_send_datagram_size
                ));
                udp_common_bind_options(&mut o, env);
                udp_common_options(&mut o, env);

                env.printer
                    .print_line(&format!("udp_server(#{{{o}}}, |listen_addr|{{sequential([",));
                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer.print_line("print_stdout(\"LISTEN proto=udp,ip=\"+listen_addr.get_ip()+\",port=\"+str(listen_addr.get_port())+\"\\n\"),");
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
                    .print_line(&format!("])}}, |{varnam}, {fromaddr}| {{",));
                env.printer.increase_indent();

                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_udp(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Endpoint::UdpConnect(_) => {}
            Endpoint::UdpBind(_) | Endpoint::UdpFd(_) | Endpoint::UdpFdNamed(_) => (),
            Endpoint::UdpServer(_) | Endpoint::UdpServerFd(_) | Endpoint::UdpServerFdNamed(_) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
