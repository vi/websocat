use std::ffi::OsStr;

use base64::Engine as _;

use crate::{cli::WebsocatArgs, scenario_executor::utils::ToNeutralAddress};

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

fn format_osstr(arg: &OsStr) -> String {
    #[cfg(any(unix, target_os = "wasi"))]
    {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(all(not(unix), target_os = "wasi"))]
        use std::os::wasi::ffi::OsStrExt;

        let x = base64::prelude::BASE64_STANDARD.encode(arg.as_bytes());
        return format!("osstr_base64_unix_bytes(\"{}\")", x);
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let b: Vec<u16> = arg.encode_wide().collect();
        let bb: Vec<u8> =
            Vec::from_iter(b.into_iter().map(|x| u16::to_le_bytes(x))).into_flattened();
        let x = base64::prelude::BASE64_STANDARD.encode(bb);

        return format!("osstr_base64_windows_utf16le(\"{}\")", x);
    }
    #[allow(unreachable_code)]
    {
        let x = base64::prelude::BASE64_STANDARD.encode(arg.as_encoded_bytes());
        return format!("osstr_base64_unchecked_encoded_bytes(\"{}\")", x);
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
                printer.print_line(&format!(
                    "listen_tcp(#{{autospawn: true, addr: {a}}}, |{varnam}, {fromaddr}| {{",
                    a = StrLit(addr),
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            Endpoint::Stdio => {
                let varnam = vars.getnewvarname("stdio");
                printer.print_line(&format!("let {varnam} = create_stdio();"));
                Ok(varnam)
            }
            Endpoint::UdpConnect(a) => {
                let varnam = vars.getnewvarname("udp");
                let maybetextmode = if opts.text { ", tag_as_text: true" } else { "" };
                printer.print_line(&format!(
                    "let {varnam} = udp_socket(#{{addr: \"{a}\"{maybetextmode}}});"
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

                printer.print_line(&format!("udp_server(#{{{o}}}, |{varnam}, {fromaddr}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::Exec(s) => {
                let var_cmd = vars.getnewvarname("cmd");
                printer.print_line(&format!("let {var_cmd} = subprocess_new({});", StrLit(s)));

                for arg in &opts.exec_args {
                    if let Some(s) = arg.to_str() {
                        printer.print_line(&format!("{var_cmd}.arg({});", StrLit(s)));
                    } else {
                        printer.print_line(&format!("{var_cmd}.arg_osstr({});", format_osstr(arg)));
                    }
                }

                self.continue_printing_cmd_or_exec(printer, vars, var_cmd, opts)
            }
            Endpoint::Cmd(s) => {
                let var_cmd = vars.getnewvarname("cmd");
                if cfg!(windows) {
                    printer.print_line(&format!("let {var_cmd} = subprocess_new(\"cmd\");"));
                    printer.print_line(&format!("{var_cmd}.arg(\"/C\");",));
                    printer.print_line(&format!(
                        "{var_cmd}.raw_windows_arg(osstr_str({}));",
                        StrLit(s)
                    ));
                } else {
                    printer.print_line(&format!("let {var_cmd} = subprocess_new(\"sh\");"));
                    printer.print_line(&format!("{var_cmd}.arg(\"-c\");",));
                    printer.print_line(&format!("{var_cmd}.arg({});", StrLit(s)));
                }

                self.continue_printing_cmd_or_exec(printer, vars, var_cmd, opts)
            }
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
                printer.print_line(&format!("let {varnam} = literal_socket_base64({});", StrLit(s)));
                Ok(varnam)
            }
        }
    }
    fn continue_printing_cmd_or_exec(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        var_cmd: String,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        if let Some(ref x) = opts.exec_chdir {
            if let Some(s) = x.to_str() {
                printer.print_line(&format!("{var_cmd}.chdir({});", StrLit(s)));
            } else {
                printer.print_line(&format!(
                    "{var_cmd}.chdir_osstr({});",
                    format_osstr(x.as_os_str())
                ));
            }
        }

        if let Some(ref x) = opts.exec_arg0 {
            if let Some(s) = x.to_str() {
                printer.print_line(&format!("{var_cmd}.arg0({});", StrLit(s)));
            } else {
                printer.print_line(&format!(
                    "{var_cmd}.arg0_osstr({});",
                    format_osstr(x.as_os_str())
                ));
            }
        }

        if let Some(x) = opts.exec_uid {
            printer.print_line(&format!("{var_cmd}.uid({x});"));
        }
        if let Some(x) = opts.exec_gid {
            printer.print_line(&format!("{var_cmd}.gid({x});"));
        }
        if let Some(x) = opts.exec_windows_creation_flags {
            printer.print_line(&format!("{var_cmd}.windows_creation_flags({x});"));
        }

        let var_chld = vars.getnewvarname("chld");
        let var_s = vars.getnewvarname("pstdio");

        printer.print_line(&format!("{var_cmd}.configure_fds(2, 2, 1);"));
        printer.print_line(&format!("let {var_chld} = {var_cmd}.execute();"));
        printer.print_line(&format!("let {var_s} = {var_chld}.socket();"));

        if opts.exec_monitor_exits {
            printer.print_line(&format!("put_hangup_part({var_s}, {var_chld}.wait());"));
        }

        Ok(var_s)
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
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            Endpoint::Stdio => {}
            Endpoint::UdpConnect(_) => {}
            Endpoint::UdpBind(_) => (),
            Endpoint::TcpConnectByLateHostname { hostname: _ } => {
                printer.decrease_indent();
                printer.print_line("})");

                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::UdpServer(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Endpoint::Exec(_) => {}
            Endpoint::Cmd(_) => {}
            Endpoint::DummyStream => {}
            Endpoint::DummyDatagrams => {}
            Endpoint::Literal(_) => {}
            Endpoint::LiteralBase64(_) => {}
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
            Overlay::WsUpgrade { uri, host } => {
                let httpclient = vars.getnewvarname("http");
                let wsframes = vars.getnewvarname("wsframes");

                printer.print_line(&format!(
                    "let {httpclient} = http1_client(#{{}}, {inner_var});"
                ));

                let mut opts = String::with_capacity(64);
                opts.push_str("url: ");
                opts.push_str(&format!("{}", StrLit(uri)));
                opts.push_str(",");

                if let Some(host) = host {
                    opts.push_str("host: ");
                    opts.push_str(&format!("{}", StrLit(&host)));
                    opts.push_str(",");
                }

                printer.print_line(&format!(
                    "ws_upgrade(#{{{opts}}}, {httpclient}, |{wsframes}| {{"
                ));
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
            Overlay::WsAccept {} => {
                let ws = vars.getnewvarname("ws");
                let hup = vars.getnewvarname("hup");
                let rq = vars.getnewvarname("rq");

                printer.print_line(&format!("http1_serve(#{{}}, {inner_var}, |{rq}, {hup}| {{"));
                printer.increase_indent();

                printer.print_line(&format!("ws_accept(#{{}}, {rq}, {hup}, |{ws}| {{"));
                printer.increase_indent();

                Ok(ws)
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
            Overlay::WsClient => {
                panic!(
                    "This overlay is supposed to be split up by specifier stack patcher before."
                );
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
            Overlay::WsUpgrade { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::WsFramer { .. } => (),
            Overlay::StreamChunks => (),
            Overlay::LineChunks => (),
            Overlay::TlsClient { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::WsAccept { .. } => {
                printer.decrease_indent();
                printer.print_line("})");

                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::Log { .. } => (),
            Overlay::WsClient => panic!(),
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
