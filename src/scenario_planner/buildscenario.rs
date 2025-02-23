use super::{
    scenarioprinter::ScenarioPrinter,
    types::{CopyingType, WebsocatInvocation},
    utils::IdentifierGenerator,
};

impl WebsocatInvocation {
    pub fn build_scenario(self, vars: &mut IdentifierGenerator) -> anyhow::Result<String> {
        let mut printer = ScenarioPrinter::new();

        let mut left: String;
        let mut right: String;

        if let Some(_tmo) = self.opts.global_timeout_ms {
            printer.print_line("race([");
            printer.increase_indent();
        }

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

        if self.opts.exit_on_hangup {
            printer.print_line(&format!(
                "try {{ handle_hangup(take_hangup_part({left}), || {{  sleep_ms(50); exit_process(0); }} ); }} catch {{}}")
            );
            printer.print_line(&format!(
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
            printer.print_line("sequential([");
            printer.increase_indent();
        }

        match self.get_copying_type() {
            CopyingType::ByteStream => {
                printer.print_line(&format!("exchange_bytes(#{{{opts}}}, {left}, {right})"));
            }
            CopyingType::Datarams => {
                printer.print_line(&format!("exchange_packets(#{{{opts}}}, {left}, {right})"));
            }
        }

        if self.opts.exit_after_one_session {
            printer.print_line(",task_wrap(||exit_process(0))])");
            printer.decrease_indent();
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

        if let Some(tmo) = self.opts.global_timeout_ms {
            if self.opts.global_timeout_force_exit {
                printer.print_line(&format!(",sequential([sleep_ms({tmo}), task_wrap(||exit_process(1))])"));
            } else {
                printer.print_line(&format!(",sleep_ms({tmo})"));
            }
            printer.decrease_indent();
            printer.print_line("])");
        }

        Ok(printer.into_result())
    }
}
