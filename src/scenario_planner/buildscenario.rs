use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::{
        Endpoint, ScenarioPrintingEnvironment, SocketType, SpecifierPosition, SpecifierStack,
        WebsocatInvocation,
    },
    utils::IdentifierGenerator,
};

impl WebsocatInvocation {
    pub fn print_scenario(
        self,
        vars: &mut IdentifierGenerator,
        printer: &mut ScenarioPrinter,
    ) -> anyhow::Result<()> {
        let mut env = ScenarioPrintingEnvironment {
            printer,
            opts: &self.opts,
            vars,
            position: SpecifierPosition::Left,
        };

        let left: String;
        let right: String;

        if let Some(tmo) = self.opts.sleep_ms_before_start {
            env.printer
                .print_line(&format!("sequential([sleep_ms({tmo}),{{"));
            env.printer.increase_indent();
        }

        if let Some(_tmo) = self.opts.global_timeout_ms {
            env.printer.print_line("race([{");
            env.printer.increase_indent();
        }

        for prepare_action in &self.beginning {
            prepare_action.begin_print(&mut env)?;
        }

        left = self.stacks.left.begin_print(&mut env)?;
        if self.stacks.right.provides_socket_type() == SocketType::SocketSender {
            // Special mode: skip most of the things and just send this socket to other Websocat session
            match self.stacks.right.innermost {
                Endpoint::RegistrySend(addr) => {
                    env.printer
                        .print_line(&format!("registry_send({}, {left})", StrLit(addr)));
                }
                _ => unreachable!(),
            }
        } else {
            env.position = SpecifierPosition::Right;
            right = self.stacks.right.begin_print(&mut env)?;

            if self.opts.exit_on_hangup {
                env.printer.print_line(&format!(
                "try {{ handle_hangup(take_hangup_part({left}), || {{  sleep_ms(50); exit_process(0); }} ); }} catch {{}}")
            );
                env.printer.print_line(&format!(
                "try {{ handle_hangup(take_hangup_part({right}), || {{  sleep_ms(50); exit_process(0); }} ); }} catch {{}}")
            );
            }

            let mut opts = String::with_capacity(64);
            if self.opts.unidirectional {
                opts.push_str("unidirectional: true,");
            }
            if self.opts.unidirectional_reverse {
                opts.push_str("unidirectional_reverse: true,");
            }
            if self.opts.exit_on_eof {
                opts.push_str("exit_on_eof: true,");
            }
            if self.opts.unidirectional_late_drop {
                opts.push_str("unidirectional_late_drop: true,");
            }
            if let Some(ref bs) = self.opts.buffer_size {
                opts.push_str(&format!("buffer_size_forward: {bs},"));
                opts.push_str(&format!("buffer_size_reverse: {bs},"));
            }

            if self.opts.exit_after_one_session {
                env.printer.print_line("sequential([");
                env.printer.increase_indent();
            }

            if let Some(ref dfd) = self.opts.exec_dup2 {
                // Special flow: direct socket FD to child process

                if matches!(
                    self.stacks.left.innermost,
                    Endpoint::Exec(..) | Endpoint::Cmd(..)
                ) {
                    anyhow::bail!("--exec-dup2 requires exec:/cmd: endpoint at the right side (second positional argument), not at the left side")
                }
                if !matches!(
                    self.stacks.right.innermost,
                    Endpoint::Exec(..) | Endpoint::Cmd(..)
                ) {
                    anyhow::bail!(
                    "--exec-dup2 requires right side (second positional argument) to be exec:/cmd:"
                )
                }

                let var_chld = env.vars.getnewvarname("chld");
                let var_fd = env.vars.getnewvarname("fd");

                env.printer
                    .print_line(&format!("let {var_fd} = get_fd({left});"));
                env.printer.print_line(&format!("if {var_fd} == -1 {{ print_stderr(\"No raw file descriptor available\") }} else {{"));
                env.printer.increase_indent();

                let mut dup2_params = String::with_capacity(16);

                dup2_params.push_str(&format!("{var_fd},["));

                for x in dfd {
                    dup2_params.push_str(&format!("{x},"));
                }
                if self.opts.exec_dup2_keep_nonblocking {
                    dup2_params.push_str("],false");
                } else {
                    dup2_params.push_str("],true");
                }

                env.printer
                    .print_line(&format!("{right}.dup2({dup2_params});"));
                if self.opts.exec_dup2_execve {
                    env.printer.print_line(&format!("{right}.execve()"));
                } else {
                    env.printer
                        .print_line(&format!("let {var_chld} = {right}.execute();"));
                    env.printer.print_line(&format!("drop({left});"));
                    env.printer
                        .print_line(&format!("hangup2task({var_chld}.wait())"));
                }
                env.printer.decrease_indent();
                env.printer.print_line("}");
            } else {
                // Usual flow: copy bytes streams / packets from left to right and back.
                match self.session_socket_type() {
                    SocketType::ByteStream => {
                        env.printer
                            .print_line(&format!("exchange_bytes(#{{{opts}}}, {left}, {right})"));
                    }
                    SocketType::Datarams => {
                        env.printer
                            .print_line(&format!("exchange_packets(#{{{opts}}}, {left}, {right})"));
                    }
                    SocketType::SocketSender => {
                        anyhow::bail!("Cannot use socketsender socket type here. It must be specified at the right side, without any overlays.")
                    }
                }
            }

            if self.opts.exit_after_one_session {
                env.printer.print_line(",task_wrap(||exit_process(0))])");
                env.printer.decrease_indent();
            }

            self.stacks.right.end_print(&mut env)?;
            env.position = SpecifierPosition::Left;
        }
        self.stacks.left.end_print(&mut env)?;

        for prepare_action in self.beginning.iter().rev() {
            prepare_action.end_print(&mut env);
        }

        if let Some(tmo) = self.opts.global_timeout_ms {
            if self.opts.global_timeout_force_exit {
                printer.print_line(&format!(
                    "}},sequential([sleep_ms({tmo}), task_wrap(||exit_process(1))])"
                ));
            } else {
                printer.print_line(&format!("}},sleep_ms({tmo})"));
            }
            printer.decrease_indent();
            printer.print_line("])");
        }
        if let Some(_tmo) = self.opts.sleep_ms_before_start {
            printer.decrease_indent();
            printer.print_line("}])");
        }

        Ok(())
    }
}

impl SpecifierStack {
    pub(super) fn begin_print(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<String> {
        let mut x: String = self.innermost.begin_print(env)?;

        for ovl in &self.overlays {
            x = ovl.begin_print(env, &x)?;
        }

        Ok(x)
    }

    pub(super) fn end_print(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
    ) -> anyhow::Result<()> {
        for ovl in self.overlays.iter().rev() {
            ovl.end_print(env);
        }

        self.innermost.end_print(env);

        Ok(())
    }
}
