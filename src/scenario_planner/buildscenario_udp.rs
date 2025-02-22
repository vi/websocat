use crate::{cli::WebsocatArgs, scenario_executor::utils1::ToNeutralAddress};

use super::{scenarioprinter::ScenarioPrinter, types::Endpoint, utils::IdentifierGenerator};

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
            Endpoint::UdpBind(a) => {
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

                let toaddr = opts.udp_bind_target_addr.unwrap_or(a.to_neutral_address());

                let mut o = String::with_capacity(64);
                o.push_str(&format!("bind: \"{a}\","));
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
            Endpoint::UdpServer(a) => {
                let varnam = vars.getnewvarname("udp");
                let fromaddr = vars.getnewvarname("from");

                let mut o = String::with_capacity(64);
                o.push_str(&format!("bind: \"{a}\","));
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

                printer.print_line(&format!("udp_server(#{{{o}}}, |{varnam}, {fromaddr}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_udp(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::UdpConnect(_) => {}
            Endpoint::UdpBind(_) => (),
            Endpoint::UdpServer(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
