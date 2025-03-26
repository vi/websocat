use crate::cli::WebsocatArgs;

use super::{
    buildscenario_exec::format_osstr,
    scenarioprinter::StrLit,
    types::{Endpoint, ScenarioPrintingEnvironment},
};

impl Endpoint {
    pub(super) fn begin_print_unix(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::UnixConnect(path) => {
                let varnam = env.vars.getnewvarname("unix");
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                env.printer
                    .print_line(&format!("connect_unix(#{{}}, {pathvar}, |{varnam}| {{",));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::UnixListen(_)
            | Endpoint::UnixListenFd(_)
            | Endpoint::UnixListenFdNamed(_) => {
                let pathvar = env.vars.getnewvarname("path");

                let mut chmod_option = "";
                let mut fd_options = "";
                let fd_options_buf;
                match self {
                    Endpoint::UnixListen(path) => {
                        if let Some(s) = path.to_str() {
                            env.printer
                                .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                        } else {
                            env.printer
                                .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                        }

                        if env.opts.unlink {
                            env.printer
                                .print_line(&format!("unlink_file({pathvar}, false);"));
                        }

                        fill_in_chmods(env.opts, &mut chmod_option);
                    }
                    Endpoint::UnixListenFd(fd) => {
                        env.printer
                            .print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",fd: {fd}");
                        fd_options = &fd_options_buf;
                    }
                    Endpoint::UnixListenFdNamed(fd) => {
                        env.printer
                            .print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",named_fd: {}", StrLit(fd));
                        fd_options = &fd_options_buf;
                    }
                    _ => unreachable!(),
                }

                let varnam = env.vars.getnewvarname("unix");
                let listenparams = env.opts.listening_parameters(env.position);

                env.printer.print_line(&format!(
                    "listen_unix(#{{{listenparams} {chmod_option} {fd_options}}}, {pathvar}, ||{{sequential([",
                ));
                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer
                        .print_line("print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = env.opts.exec_after_listen {
                    env.printer.print_line(&format!("system({}),", StrLit(x)));
                }

                env.printer.decrease_indent();
                env.printer.print_line(&format!("])}},  |{varnam}| {{",));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractConnect(path) => {
                let varnam = env.vars.getnewvarname("unix");
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                #[allow(clippy::literal_string_with_formatting_args)]
                env.printer.print_line(&format!(
                    "connect_unix(#{{abstract:true}}, {pathvar}, |{varnam}| {{",
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractListen(path) => {
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                let varnam = env.vars.getnewvarname("unix");
                let listenparams = env.opts.listening_parameters(env.position);

                env.printer.print_line(&format!(
                    "listen_unix(#{{abstract: true, {listenparams} }}, {pathvar}, ||{{sequential([",
                ));
                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer
                        .print_line("print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = env.opts.exec_after_listen {
                    env.printer.print_line(&format!("system({}),", StrLit(x)));
                }

                env.printer.decrease_indent();
                env.printer.print_line(&format!("])}},  |{varnam}| {{",));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::SeqpacketConnect(path) => {
                let varnam = env.vars.getnewvarname("unix");
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                let mut text_option = "";
                if env.opts.text {
                    text_option = "text: true";
                }

                env.printer.print_line(&format!(
                    "connect_seqpacket(#{{ max_send_datagram_size: {} , {text_option}, }}, {pathvar}, |{varnam}| {{",
                    env.opts.seqpacket_max_send_datagram_size,
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::SeqpacketListen(_)
            | Endpoint::SeqpacketListenFd(_)
            | Endpoint::SeqpacketListenFdNamed(_) => {
                let pathvar = env.vars.getnewvarname("path");

                let mut chmod_option = "";
                let mut fd_options = "";
                let fd_options_buf;
                match self {
                    Endpoint::SeqpacketListen(path) => {
                        if let Some(s) = path.to_str() {
                            env.printer
                                .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                        } else {
                            env.printer
                                .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                        }

                        if env.opts.unlink {
                            env.printer
                                .print_line(&format!("unlink_file({pathvar}, false);"));
                        }

                        fill_in_chmods(env.opts, &mut chmod_option);
                    }
                    Endpoint::SeqpacketListenFd(fd) => {
                        env.printer
                            .print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",fd: {fd}");
                        fd_options = &fd_options_buf;
                    }
                    Endpoint::SeqpacketListenFdNamed(fd) => {
                        env.printer
                            .print_line(&format!("let {pathvar} = osstr_str(\"\");"));
                        fd_options_buf = format!(",named_fd: {}", StrLit(fd));
                        fd_options = &fd_options_buf;
                    }
                    _ => unreachable!(),
                }

                let varnam = env.vars.getnewvarname("unix");

                let mut text_option = "";
                if env.opts.text {
                    text_option = ", text: true";
                }
                let listenparams = env.opts.listening_parameters(env.position);

                env.printer.print_line(&format!(
                    "listen_seqpacket(#{{{listenparams} {chmod_option} {text_option} {fd_options} , max_send_datagram_size: {} }}, {pathvar}, ||{{sequential([",
                    env.opts.seqpacket_max_send_datagram_size,
                ));
                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer
                        .print_line("print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = env.opts.exec_after_listen {
                    env.printer.print_line(&format!("system({}),", StrLit(x)));
                }

                env.printer.decrease_indent();
                env.printer.print_line(&format!("])}},  |{varnam}| {{",));
                env.printer.increase_indent();

                Ok(varnam)
            }
            Endpoint::AbstractSeqpacketConnect(path) => {
                let varnam = env.vars.getnewvarname("unix");
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }
                let mut text_option = "";
                if env.opts.text {
                    text_option = ", text: true";
                }

                env.printer.print_line(&format!(
                    "connect_seqpacket(#{{abstract:true {text_option}, max_send_datagram_size: {}}}, {pathvar}, |{varnam}| {{",
                    env.opts.seqpacket_max_send_datagram_size,
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::AbstractSeqpacketListen(path) => {
                let pathvar = env.vars.getnewvarname("path");
                if let Some(s) = path.to_str() {
                    env.printer
                        .print_line(&format!("let {pathvar} = osstr_str({});", StrLit(s)));
                } else {
                    env.printer
                        .print_line(&format!("let {pathvar} = {};", format_osstr(path)));
                }

                let varnam = env.vars.getnewvarname("unix");

                let mut text_option = "";
                if env.opts.text {
                    text_option = ", text: true";
                }
                let listenparams = env.opts.listening_parameters(env.position);

                env.printer.print_line(&format!(
                    "listen_seqpacket(#{{abstract:true, {listenparams} {text_option} , max_send_datagram_size: {} }}, {pathvar}, ||{{sequential([",
                    env.opts.seqpacket_max_send_datagram_size,
                ));

                env.printer.increase_indent();

                if env.opts.stdout_announce_listening_ports {
                    env.printer
                        .print_line("print_stdout(\"LISTEN proto=unix\\n\"),");
                }
                if let Some(ref x) = env.opts.exec_after_listen {
                    env.printer.print_line(&format!("system({}),", StrLit(x)));
                }

                env.printer.decrease_indent();
                env.printer.print_line(&format!("])}},  |{varnam}| {{",));
                env.printer.increase_indent();

                Ok(varnam)
            }
            Endpoint::AsyncFd(fd) => {
                let asyncfd = env.vars.getnewvarname("asyncfd");

                let force = env.opts.async_fd_force;

                env.printer
                    .print_line(&format!("let {asyncfd} = async_fd({fd}, {force});"));
                Ok(asyncfd)
            }
            _ => panic!(),
        }
    }

    pub(super) fn end_print_unix(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
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
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::AsyncFd(_) => {}
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
