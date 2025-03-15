use crate::cli::CustomHeader;

use super::{
    scenarioprinter::StrLit,
    types::{Endpoint, Overlay, ScenarioPrintingEnvironment},
};

impl Endpoint {
    pub(super) fn begin_print_ws(
        &self,
        _env: &mut ScenarioPrintingEnvironment<'_>,
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

    pub(super) fn end_print_ws(&self, _env: &mut ScenarioPrintingEnvironment<'_>) {
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
        env: &mut ScenarioPrintingEnvironment<'_>,
        inner_var: &str,
    ) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade { uri, host } => {
                let httpclient = env.vars.getnewvarname("http");
                let wsframes = env.vars.getnewvarname("wsframes");

                env.printer.print_line(&format!(
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

                if env.opts.ws_dont_check_headers {
                    oo.push_str("lax: true,")
                }
                if env.opts.ws_omit_headers {
                    oo.push_str("omit_headers: true,")
                }

                let mut ch = String::new();
                if let Some(ref x) = env.opts.origin {
                    ch.push_str(&format!("\"Origin\":{},", StrLit(x)));
                }
                if let Some(ref x) = env.opts.ua {
                    ch.push_str(&format!("\"User-Agent\":{},", StrLit(x)));
                }
                for CustomHeader { name, value } in &env.opts.header {
                    ch.push_str(&format!("{}:{},", StrLit(name), StrLit(value)))
                }
                if let Some(ref proto) = env.opts.protocol {
                    ch.push_str(&format!("\"Sec-WebSocket-Protocol\":{},", StrLit(proto)))
                }

                env.printer.print_line(&format!(
                    "ws_upgrade(#{{{oo}}}, #{{{ch}}}, {httpclient}, |{wsframes}| {{"
                ));
                env.printer.increase_indent();

                Ok(wsframes)
            }
            Overlay::WsFramer { client_mode } => {
                let ws = env.vars.getnewvarname("ws");

                let mut oo = String::with_capacity(0);
                if env.opts.no_close {
                    oo.push_str("no_close_frame: true,")
                }
                if env.opts.ws_no_flush {
                    oo.push_str("no_flush_after_each_message: true,")
                }
                if env.opts.ws_ignore_invalid_masks {
                    oo.push_str("ignore_masks: true,")
                }
                if env.opts.ws_no_auto_buffer {
                    oo.push_str("no_auto_buffer_wrap: true,")
                }
                if env.opts.ws_shutdown_socket_on_eof {
                    oo.push_str("shutdown_socket_on_eof: true,")
                }
                if let Some(mp) = env.opts.inhibit_pongs {
                    oo.push_str(&format!("max_ping_replies: {mp},"));
                }

                env.printer.print_line(&format!(
                    "let {ws} = ws_wrap(#{{{oo}client: {client_mode}}}, {inner_var});"
                ));

                Ok(ws)
            }

            Overlay::WsAccept {} => {
                let ws = env.vars.getnewvarname("ws");
                let hup = env.vars.getnewvarname("hup");
                let fd = env.vars.getnewvarname("fd");
                let rq = env.vars.getnewvarname("rq");

                env.printer.print_line(&format!(
                    "http1_serve(#{{}}, {inner_var}, |{rq}, {hup}, {fd}| {{"
                ));
                env.printer.increase_indent();

                let mut oo = String::new();

                if env.opts.ws_dont_check_headers {
                    oo.push_str("lax: true,")
                }
                if env.opts.ws_omit_headers {
                    oo.push_str("omit_headers: true,")
                }
                if env.opts.server_protocol_choose_first {
                    oo.push_str("protocol_choose_first: true,");
                }
                if let Some(ref x) = env.opts.server_protocol {
                    oo.push_str(&format!("choose_protocol: {},", StrLit(x)));
                    if !env.opts.server_protocol_lax {
                        oo.push_str("require_protocol: true,");
                    }
                }

                let mut ch = String::new();
                for CustomHeader { name, value } in &env.opts.server_header {
                    ch.push_str(&format!("{}:{},", StrLit(name), StrLit(value)))
                }

                env.printer.print_line(&format!(
                    "ws_accept(#{{{oo}}}, #{{{ch}}}, {rq}, {hup}, {fd}, |{ws}| {{"
                ));
                env.printer.increase_indent();

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

    pub(super) fn end_print_ws(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Overlay::WsUpgrade { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Overlay::WsFramer { .. } => (),
            Overlay::WsClient => panic!(),
            Overlay::WsServer => panic!(),
            Overlay::WsAccept { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");

                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}
