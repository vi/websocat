use super::{
    buildscenario_exec::format_osstr,
    scenarioprinter::StrLit,
    types::{CopyingType, Endpoint, ScenarioPrintingEnvironment},
};

impl Endpoint {
    pub(super) fn begin_print(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        match self {
            Endpoint::TcpConnectByIp(..)
            | Endpoint::TcpConnectByEarlyHostname { .. }
            | Endpoint::TcpListen { .. }
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..)
            | Endpoint::TcpConnectByLateHostname { .. } => self.begin_print_tcp(env),
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                self.begin_print_ws(env)
            }
            Endpoint::Stdio => {
                let varnam = env.vars.getnewvarname("stdio");
                env.printer
                    .print_line(&format!("let {varnam} = stdio_socket();"));
                Ok(varnam)
            }
            Endpoint::Zero => {
                let varnam = env.vars.getnewvarname("zero");
                env.printer
                    .print_line(&format!("let {varnam} = zero_socket();"));
                Ok(varnam)
            }
            Endpoint::Random => {
                let varnam = env.vars.getnewvarname("random");
                let fast = env.opts.random_fast;
                env.printer
                    .print_line(&format!("let {varnam} = random_socket(#{{fast: {fast}}});"));
                Ok(varnam)
            }
            Endpoint::UdpConnect(..)
            | Endpoint::UdpBind(..)
            | Endpoint::UdpServer(..)
            | Endpoint::UdpServerFd(_)
            | Endpoint::UdpServerFdNamed(_)
            | Endpoint::UdpFd(_)
            | Endpoint::UdpFdNamed(_) => self.begin_print_udp(env),
            Endpoint::Exec(..) | Endpoint::Cmd(..) => self.begin_print_exec(env),
            Endpoint::DummyStream => {
                let varnam = env.vars.getnewvarname("dummy");
                env.printer
                    .print_line(&format!("let {varnam} = dummy_stream_socket();"));
                if env.opts.dummy_hangup {
                    env.printer.print_line(&format!(
                        "put_hangup_part({varnam}, pre_triggered_hangup_handle());"
                    ));
                }
                Ok(varnam)
            }
            Endpoint::DummyDatagrams => {
                let varnam = env.vars.getnewvarname("dummy");
                env.printer
                    .print_line(&format!("let {varnam} = dummy_datagram_socket();"));
                if env.opts.dummy_hangup {
                    env.printer.print_line(&format!(
                        "put_hangup_part({varnam}, pre_triggered_hangup_handle());"
                    ));
                }
                Ok(varnam)
            }
            Endpoint::Literal(s) => {
                let varnam = env.vars.getnewvarname("lit");
                env.printer
                    .print_line(&format!("let {varnam} = literal_socket({});", StrLit(s)));
                Ok(varnam)
            }
            Endpoint::LiteralBase64(s) => {
                let varnam = env.vars.getnewvarname("lit");
                env.printer.print_line(&format!(
                    "let {varnam} = literal_socket_base64({});",
                    StrLit(s)
                ));
                Ok(varnam)
            }
            Endpoint::UnixConnect(..)
            | Endpoint::UnixListen(..)
            | Endpoint::AbstractConnect(_)
            | Endpoint::AbstractListen(_)
            | Endpoint::UnixListenFd(_)
            | Endpoint::UnixListenFdNamed(_)
            | Endpoint::AsyncFd(_)
            | Endpoint::SeqpacketConnect(_)
            | Endpoint::SeqpacketListen(_)
            | Endpoint::SeqpacketListenFd(_)
            | Endpoint::SeqpacketListenFdNamed(_)
            | Endpoint::AbstractSeqpacketConnect(_)
            | Endpoint::AbstractSeqpacketListen(_) => self.begin_print_unix(env),
            Endpoint::MockStreamSocket(s) => {
                let varnam = env.vars.getnewvarname("mock");
                env.printer.print_line(&format!(
                    "let {varnam} = mock_stream_socket({});",
                    StrLit(s)
                ));
                Ok(varnam)
            }
            Endpoint::RegistryStreamListen(addr) => {
                let listenparams = env.opts.listening_parameters(env.position);
                let varnam = env.vars.getnewvarname("reg");
                env.printer.print_line(&format!(
                    "listen_registry_stream(#{{{listenparams}, addr: {a}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::RegistryStreamConnect(addr) => {
                let mbs = env.opts.registry_connect_bufsize;
                let varnam = env.vars.getnewvarname("reg");
                env.printer.print_line(&format!(
                    "connect_registry_stream(#{{addr: {a}, max_buf_size: {mbs}}}, |{varnam}| {{",
                    a = StrLit(addr)
                ));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::SimpleReuserEndpoint(varname, specifier_stack) => {
                let slot = env.vars.getnewvarname("slot");
                let conn = env.vars.getnewvarname("reuseconn");
                let strict_mode = !env.opts.reuser_tolerate_torn_msgs;
                env.printer.print_line(&format!(
                    "{varname}.maybe_init_then_connect(#{{connect_again: true, disconnect_on_broken_message: {strict_mode}}}, |{slot}| {{"
                ));
                env.printer.increase_indent();

                let x = specifier_stack.begin_print(env)?;

                env.printer.print_line(&format!("{slot}.send({x})"));

                specifier_stack.end_print(env)?;

                env.printer.decrease_indent();
                env.printer.print_line(&format!("}}, |{conn}| {{"));
                env.printer.increase_indent();
                Ok(conn)
            }
            Endpoint::ReadFile(p) | Endpoint::WriteFile(p) | Endpoint::AppendFile(p) => {
                let mut oo = String::with_capacity(32);

                match self {
                    Endpoint::ReadFile(..) => {
                        oo += "write: false";
                    }
                    Endpoint::WriteFile(..) => {
                        oo += "write: true";
                        if env.opts.write_file_no_overwrite {
                            oo += ", no_overwrite: true";
                        }
                        if env.opts.write_file_auto_rename {
                            oo += ", auto_rename: true";
                        }
                    }
                    Endpoint::AppendFile(..) => {
                        oo += "append: true";
                    }
                    _ => unreachable!(),
                }

                let pathnam = env.vars.getnewvarname("path");
                let varnam = env.vars.getnewvarname("file");

                env.printer
                    .print_line(&format!("let {pathnam} = {};", format_osstr(p)));

                env.printer
                    .print_line(&format!("file_socket(#{{{oo}}}, {pathnam}, |{varnam}| {{",));
                env.printer.increase_indent();
                Ok(varnam)
            }
            Endpoint::WriteSplitoff { read, write } => {
                let ct = self.get_copying_type();

                let varnam = env.vars.getnewvarname("writesplitoff");
                let readslot = env.vars.getnewvarname("readslot");
                let writeslot = env.vars.getnewvarname("writeslot");
                let twosock = env.vars.getnewvarname("twosock");

                env.printer
                    .print_line(&format!("init_in_parallel([ |{readslot}| {{"));
                env.printer.increase_indent();

                let readresult = read.begin_print(env)?;
                env.printer
                    .print_line(&format!("{readslot}.send({readresult})"));
                read.end_print(env)?;

                env.printer.decrease_indent();
                env.printer.print_line(&format!("}},|{writeslot}| {{"));
                env.printer.increase_indent();

                let writeresult = write.begin_print(env)?;
                env.printer
                    .print_line(&format!("{writeslot}.send({writeresult})"));
                write.end_print(env)?;

                env.printer.decrease_indent();

                env.printer.print_line(&format!("}}], |{twosock}| {{"));
                env.printer.increase_indent();

                match ct {
                    CopyingType::ByteStream => {
                        env.printer.print_line(&format!("let {varnam} = combine_read_and_write_bytestream({twosock}[0], {twosock}[1]);"));
                    }
                    CopyingType::Datarams => {
                        env.printer.print_line(&format!("let {varnam} = combine_read_and_write_datagram({twosock}[0], {twosock}[1]);"));
                    }
                }

                if !env.opts.write_splitoff_omit_shutdown {
                    env.printer
                        .print_line(&format!("shutdown_and_drop({twosock}[0]);"));
                    env.printer.print_line(&format!("drop({twosock}[1]);"));
                }

                Ok(varnam)
            }
        }
    }

    pub(super) fn end_print(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Endpoint::TcpConnectByIp(..)
            | Endpoint::TcpConnectByEarlyHostname { .. }
            | Endpoint::TcpListen { .. }
            | Endpoint::TcpListenFd(..)
            | Endpoint::TcpListenFdNamed(..)
            | Endpoint::TcpConnectByLateHostname { .. } => self.end_print_tcp(env),
            Endpoint::WsUrl(..) | Endpoint::WssUrl(..) | Endpoint::WsListen(..) => {
                self.end_print_ws(env)
            }
            Endpoint::Stdio => {}
            Endpoint::UdpConnect(_)
            | Endpoint::UdpBind(_)
            | Endpoint::UdpServer(_)
            | Endpoint::UdpServerFd(_)
            | Endpoint::UdpServerFdNamed(_)
            | Endpoint::UdpFd(_)
            | Endpoint::UdpFdNamed(_) => self.end_print_udp(env),
            Endpoint::Exec(_) | Endpoint::Cmd(_) => self.end_print_exec(env),
            Endpoint::DummyStream => {}
            Endpoint::DummyDatagrams => {}
            Endpoint::Literal(_) => {}
            Endpoint::LiteralBase64(_) => {}
            Endpoint::UnixConnect(_)
            | Endpoint::UnixListen(_)
            | Endpoint::AbstractConnect(_)
            | Endpoint::AbstractListen(_)
            | Endpoint::UnixListenFd(_)
            | Endpoint::UnixListenFdNamed(_)
            | Endpoint::AsyncFd(_)
            | Endpoint::SeqpacketConnect(_)
            | Endpoint::SeqpacketListen(_)
            | Endpoint::SeqpacketListenFd(_)
            | Endpoint::SeqpacketListenFdNamed(_)
            | Endpoint::AbstractSeqpacketConnect(_)
            | Endpoint::AbstractSeqpacketListen(_) => self.end_print_unix(env),
            Endpoint::MockStreamSocket(_) => {}
            Endpoint::RegistryStreamListen(_) | Endpoint::RegistryStreamConnect(_) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::SimpleReuserEndpoint(..) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::ReadFile(..) | Endpoint::WriteFile(..) | Endpoint::AppendFile(..) => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Endpoint::Random => {}
            Endpoint::Zero => {}
            Endpoint::WriteSplitoff { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
        }
    }
}
