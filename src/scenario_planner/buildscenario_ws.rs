use crate::cli::{CustomHeader, WebsocatArgs};

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::{Endpoint, Overlay},
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print_ws(
        &self,
        _printer: &mut ScenarioPrinter,
        _vars: &mut IdentifierGenerator,
        _opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_ws(&self, _printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                panic!(
                    "This endpoint is supposed to be split up by specifier stack patcher before."
                );
            }
            _ => panic!(),
        }
    }
}

impl Overlay {
    pub(super) fn begin_print_ws(
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

                let mut oo = String::with_capacity(64);
                oo.push_str("url: ");
                oo.push_str(&format!("{}", StrLit(uri)));
                oo.push(',');

                if let Some(host) = host {
                    oo.push_str("host: ");
                    oo.push_str(&format!("{}", StrLit(&host)));
                    oo.push(',');
                }

                if opts.ws_dont_check_headers {
                    oo.push_str("lax: true,")
                }
                if opts.ws_omit_headers {
                    oo.push_str("omit_headers: true,")
                }

                let mut ch = String::new();
                for CustomHeader { name, value } in &opts.header {
                    ch.push_str(&format!("{}:{},", StrLit(name), StrLit(value)))
                }
                if let Some(ref proto) = opts.protocol {
                    ch.push_str(&format!("\"Sec-WebSocket-Protocol\":{},", StrLit(proto)))
                }

                printer.print_line(&format!(
                    "ws_upgrade(#{{{oo}}}, #{{{ch}}}, {httpclient}, |{wsframes}| {{"
                ));
                printer.increase_indent();

                Ok(wsframes)
            }
            Overlay::WsFramer { client_mode } => {
                let ws = vars.getnewvarname("ws");

                let mut oo = String::with_capacity(0);
                if opts.no_close {
                    oo.push_str("no_close_frame: true,")
                }
                if opts.ws_no_flush {
                    oo.push_str("no_flush_after_each_message: true,")
                }
                if opts.ws_ignore_invalid_masks {
                    oo.push_str("ignore_masks: true,")
                }
                if opts.ws_no_auto_buffer {
                    oo.push_str("no_auto_buffer_wrap: true,")
                }
                if opts.ws_shutdown_socket_on_eof {
                    oo.push_str("shutdown_socket_on_eof: true,")
                }
                if let Some(mp) = opts.inhibit_pongs {
                    oo.push_str(&format!("max_ping_replies: {mp},"));
                }

                printer.print_line(&format!(
                    "let {ws} = ws_wrap(#{{{oo}client: {client_mode}}}, {inner_var});"
                ));

                Ok(ws)
            }

            Overlay::WsAccept {} => {
                let ws = vars.getnewvarname("ws");
                let hup = vars.getnewvarname("hup");
                let rq = vars.getnewvarname("rq");

                printer.print_line(&format!("http1_serve(#{{}}, {inner_var}, |{rq}, {hup}| {{"));
                printer.increase_indent();

                let mut oo = String::new();

                if opts.ws_dont_check_headers {
                    oo.push_str("lax: true,")
                }
                if opts.ws_omit_headers {
                    oo.push_str("omit_headers: true,")
                }
                if opts.server_protocol_choose_first {
                    oo.push_str("protocol_choose_first: true,");
                }
                if let Some(ref x) = opts.server_protocol {
                    oo.push_str(&format!("choose_protocol: {},", StrLit(x)));
                    if !opts.server_protocol_lax {
                        oo.push_str("require_protocol: true,");
                    }
                }

                let mut ch = String::new();
                for CustomHeader { name, value } in &opts.server_header {
                    ch.push_str(&format!("{}:{},", StrLit(name), StrLit(value)))
                }

                printer.print_line(&format!(
                    "ws_accept(#{{{oo}}}, #{{{ch}}}, {rq}, {hup}, |{ws}| {{"
                ));
                printer.increase_indent();

                Ok(ws)
            }
            Overlay::WsClient => {
                panic!(
                    "This overlay is supposed to be split up by specifier stack patcher before."
                );
            }
            Overlay::WsServer => {
                panic!(
                    "This overlay is supposed to be split up by specifier stack patcher before."
                );
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_ws(&self, printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::WsFramer { .. } => (),
            Overlay::WsClient => panic!(),
            Overlay::WsServer => panic!(),
            Overlay::WsAccept { .. } => {
                printer.decrease_indent();
                printer.print_line("})");

                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
