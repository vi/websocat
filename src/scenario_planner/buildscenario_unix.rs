use crate::cli::WebsocatArgs;

use super::{
    buildscenario_exec::format_osstr,
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Endpoint,
    utils::IdentifierGenerator,
};

impl Endpoint {
    pub(super) fn begin_print_unix(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::UnixConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                printer.print_line(&format!("connect_unix(#{{}}, {pathvar}, |{varnam}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::UnixListen(_)
            | Endpoint::UnixListenFd(_)
            | Endpoint::UnixListenFdNamed(_) => {
                let pathvar = vars.getnewvarname("path");

                let mut chmod_option = "";
                let mut fd_options = "";
                let fd_options_buf;
                match self {
                    Endpoint::UnixListen(path) => {
                        if let Some(s) = path.to_str() {
                            printer
                                .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                        } else {
                            printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                        }

                        if opts.unlink {
                            printer.print_line(&format!("unlink_file({pathvar}, false);"));
                        }

                        fill_in_chmods(opts, &mut chmod_option);
                    }
                    Endpoint::UnixListenFd(fd) => {
                        printer.print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",fd: {fd}");
                        fd_options = &fd_options_buf;
                    }
                    Endpoint::UnixListenFdNamed(fd) => {
                        printer.print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",named_fd: {}", StrLit(fd));
                        fd_options = &fd_options_buf;
                    }
                    _ => unreachable!(),
                }

                let varnam = vars.getnewvarname("unix");
                let listenparams = opts.listening_parameters();

                printer.print_line(&format!(
                    "listen_unix(#{{{listenparams} {chmod_option} {fd_options}}}, {pathvar}, ||{{sequential([",
                ));
                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line(&"print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = opts.exec_after_listen {
                    printer.print_line(&format!("system({}),", StrLit(x)));
                }

                printer.decrease_indent();
                printer.print_line(&format!("])}},  |{varnam}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                printer.print_line(&format!(
                    "connect_unix(#{{abstract:true}}, {pathvar}, |{varnam}| {{",
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractListen(path) => {
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                let varnam = vars.getnewvarname("unix");
                let listenparams = opts.listening_parameters();

                printer.print_line(&format!(
                    "listen_unix(#{{abstract: true, {listenparams} }}, {pathvar}, ||{{sequential([",
                ));
                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line(&"print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = opts.exec_after_listen {
                    printer.print_line(&format!("system({}),", StrLit(x)));
                }

                printer.decrease_indent();
                printer.print_line(&format!("])}},  |{varnam}| {{",));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::SeqpacketConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                let mut text_option = "";
                if opts.text {
                    text_option = "text: true";
                }

                printer.print_line(&format!(
                    "connect_seqpacket(#{{ max_send_datagram_size: {} , {text_option}, }}, {pathvar}, |{varnam}| {{",
                    opts.seqpacket_max_send_datagram_size,
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::SeqpacketListen(_)
            | Endpoint::SeqpacketListenFd(_)
            | Endpoint::SeqpacketListenFdNamed(_) => {
                let pathvar = vars.getnewvarname("path");

                let mut chmod_option = "";
                let mut fd_options = "";
                let fd_options_buf;
                match self {
                    Endpoint::SeqpacketListen(path) => {
                        if let Some(s) = path.to_str() {
                            printer
                                .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                        } else {
                            printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                        }

                        if opts.unlink {
                            printer.print_line(&format!("unlink_file({pathvar}, false);"));
                        }

                        fill_in_chmods(opts, &mut chmod_option);
                    }
                    Endpoint::SeqpacketListenFd(fd) => {
                        printer.print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",fd: {fd}");
                        fd_options = &fd_options_buf;
                    }
                    Endpoint::SeqpacketListenFdNamed(fd) => {
                        printer.print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",named_fd: {}", StrLit(fd));
                        fd_options = &fd_options_buf;
                    }
                    _ => unreachable!(),
                }

                let varnam = vars.getnewvarname("unix");

                let mut text_option = "";
                if opts.text {
                    text_option = ", text: true";
                }
                let listenparams = opts.listening_parameters();

                printer.print_line(&format!(
                    "listen_seqpacket(#{{{listenparams} {chmod_option} {text_option} {fd_options} , max_send_datagram_size: {} }}, {pathvar}, ||{{sequential([",
                    opts.seqpacket_max_send_datagram_size,
                ));
                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line(&"print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = opts.exec_after_listen {
                    printer.print_line(&format!("system({}),", StrLit(x)));
                }

                printer.decrease_indent();
                printer.print_line(&format!("])}},  |{varnam}| {{",));
                printer.increase_indent();

                Ok(varnam)
            }
            Endpoint::AbstractSeqpacketConnect(path) => {
                let varnam = vars.getnewvarname("unix");
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                let mut text_option = "";
                if opts.text {
                    text_option = ", text: true";
                }

                printer.print_line(&format!(
                    "connect_seqpacket(#{{abstract:true {text_option}, max_send_datagram_size: {}}}, {pathvar}, |{varnam}| {{",
                    opts.seqpacket_max_send_datagram_size,
                ));
                printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractSeqpacketListen(path) => {
                let pathvar = vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    printer.print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    printer.print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                let varnam = vars.getnewvarname("unix");

                let mut text_option = "";
                if opts.text {
                    text_option = ", text: true";
                }
                let listenparams = opts.listening_parameters();

                printer.print_line(&format!(
                    "listen_seqpacket(#{{abstract:true, {listenparams} {text_option} , max_send_datagram_size: {} }}, {pathvar}, ||{{sequential([",
                    opts.seqpacket_max_send_datagram_size,
                ));

                printer.increase_indent();

                if opts.stdout_announce_listening_ports {
                    printer.print_line(&"print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = opts.exec_after_listen {
                    printer.print_line(&format!("system({}),", StrLit(x)));
                }

                printer.decrease_indent();
                printer.print_line(&format!("])}},  |{varnam}| {{",));
                printer.increase_indent();

                Ok(varnam)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_unix(&self, printer: &mut ScenarioPrinter) {
        match self {
            Endpoint::UnixConnect(_)
            | Endpoint::UnixListen(_)
            | Endpoint::AbstractConnect(_)
            | Endpoint::AbstractListen(_)
            | Endpoint::UnixListenFd(_)
            | Endpoint::UnixListenFdNamed(_)
            | Endpoint::SeqpacketConnect(_)
            | Endpoint::SeqpacketListen(_)
            | Endpoint::SeqpacketListenFd(_)
            | Endpoint::SeqpacketListenFdNamed(_)
            | Endpoint::AbstractSeqpacketConnect(_)
            | Endpoint::AbstractSeqpacketListen(_) => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            _ => panic!(),
        }
    }
}

fn fill_in_chmods(opts: &WebsocatArgs, chmod_option: &mut &str) {
    if opts.chmod_owner {
        *chmod_option = ", chmod: 0o600";
    } else if opts.chmod_group {
        *chmod_option = ", chmod: 0o660";
    } else if opts.chmod_everyone {
        *chmod_option = ", chmod: 0o666";
    }
}
