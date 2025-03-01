use crate::{
    cli::WebsocatArgs,
    scenario_executor::utils1::{ToNeutralAddress, NEUTRAL_SOCKADDR6},
};

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Endpoint,
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print_udp(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::UdpConnect(a) => {
                let varnam = vars.getnewvarname("udp");
                let maybetextmode = if opts.text { ", tag_as_text: true" } else { "" };
                printer.print_line(&format!(
                    "let {varnam} = udp_socket(#{{addr: \"{a}\", max_send_datagram_size: {} {maybetextmode}}});",
                    opts.udp_max_send_datagram_size
                ));
                Ok(varnam)
            }
            Endpoint::UdpBind(_) | Endpoint::UdpFd(_) | Endpoint::UdpFdNamed(_) => {
                let varnam = vars.getnewvarname("udp");

                let mut udp_bind_redirect_to_last_seen_address =
                    opts.udp_bind_redirect_to_last_seen_address;
                let mut udp_bind_connect_to_first_seen_address =
                    opts.udp_bind_connect_to_first_seen_address;

                if opts.udp_bind_restrict_to_one_address && opts.udp_bind_target_addr.is_none() {
                    anyhow::bail!("It is meaningless to --udp-bind-restrict-to-one-address without also specifying --udp-bind-target-addr")
                }
                if opts.udp_bind_restrict_to_one_address
                    && (opts.udp_bind_connect_to_first_seen_address
                        || opts.udp_bind_redirect_to_last_seen_address)
                {
                    anyhow::bail!("It is meaningless to use --udp-bind-restrict-to-one-address with another option to react at new incoming addresses")
                }

                if opts.udp_bind_target_addr.is_none() {
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

                let toaddr = opts.udp_bind_target_addr.unwrap_or(neutral);
                o.push_str(&format!("addr: \"{toaddr}\","));
                o.push_str(&format!("sendto_mode: true,"));

                if !opts.udp_bind_restrict_to_one_address {
                    o.push_str(&format!("allow_other_addresses: true,"));
                }
                if udp_bind_redirect_to_last_seen_address {
                    o.push_str(&format!("redirect_to_last_seen_address: true,"));
                }
                if udp_bind_connect_to_first_seen_address {
                    o.push_str(&format!("connect_to_first_seen_address: true,"));
                }
                if opts.udp_bind_inhibit_send_errors {
                    o.push_str(&format!("inhibit_send_errors: true,"));
                }
                o.push_str(&format!(
                    "max_send_datagram_size: {},",
                    opts.udp_max_send_datagram_size
                ));

                if opts.text {
                    o.push_str(&format!("tag_as_text: true,"));
                }

                printer.print_line(&format!("let {varnam} = udp_socket(#{{{o}}});"));

                Ok(varnam)
            }
            Endpoint::UdpServer(_) | Endpoint::UdpServerFd(_) | Endpoint::UdpServerFdNamed(_) => {
                let varnam = vars.getnewvarname("udp");
                let fromaddr = vars.getnewvarname("from");

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

                if opts.udp_bind_inhibit_send_errors {
                    o.push_str(&format!("inhibit_send_errors: true,"));
                }
                if opts.text {
                    o.push_str(&format!("tag_as_text: true,"));
                }
                if opts.udp_server_backpressure {
                    o.push_str(&format!("backpressure: true,"));
                }
                if let Some(x) = opts.udp_server_timeout_ms {
                    o.push_str(&format!("timeout_ms: {x},"));
                }
                if let Some(x) = opts.udp_server_max_clients {
                    o.push_str(&format!("max_clients: {x},"));
                }
                if let Some(x) = opts.udp_server_buffer_size {
                    o.push_str(&format!("buffer_size: {x},"));
                }
                if let Some(x) = opts.udp_server_qlen {
                    o.push_str(&format!("qlen: {x},"));
                }
                o.push_str(&format!(
                    "max_send_datagram_size: {},",
                    opts.udp_max_send_datagram_size
                ));

                printer.print_line(&format!("udp_server(#{{{o}}}, |listen_addr|{{sequential([",));
                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line(&"print_stdout(\"LISTEN proto=udp,ip=\"+listen_addr.get_ip()+\",port=\"+str(listen_addr.get_port())+\"\\n\"),");
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
                printer.print_line(&format!("])}}, |{varnam}, {fromaddr}| {{",));
                printer.increase_indent();

                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_udp(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::UdpConnect(_) => {}
            Endpoint::UdpBind(_) | Endpoint::UdpFd(_) | Endpoint::UdpFdNamed(_) => (),
            Endpoint::UdpServer(_) | Endpoint::UdpServerFd(_) | Endpoint::UdpServerFdNamed(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
