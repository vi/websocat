use super::{SpecifierClass, SpecifierStack, WebsocatConfiguration2};
use std::rc::Rc;
use std::str::FromStr;

#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum StdioUsageStatus {
    /// Does not use standard input or output at all
    None,
    /// Uses a reuser for connecting multiple peers at stdio, not distinguishing between IsItself and Indirectly
    WithReuser,
    /// Stdio wrapped into something (but not the reuser)
    Indirectly,
    /// Is the `-` or `stdio:` or `threadedstdio:` itself.
    IsItself,
}

trait ClassExt {
    fn is_stdio(&self) -> bool;
    fn is_reuser(&self) -> bool;
}

#[cfg_attr(rustfmt, rustfmt_skip)]
impl ClassExt for Rc<SpecifierClass> {
    fn is_stdio(&self) -> bool {
        [
            "StdioClass",
            "ThreadedStdioClass",
            "ThreadedStdioSubstituteClass",
        ].contains(&self.get_name())
    }
    fn is_reuser(&self) -> bool {
        [
            "ReuserClass",
            "BroadcastReuserClass",
        ].contains(&self.get_name())
    }
}

pub trait SpecifierStackExt {
    fn stdio_usage_status(&self) -> StdioUsageStatus;
    fn reuser_count(&self) -> usize;
    fn contains(&self, t: &'static str) -> bool;
    fn is_multiconnect(&self) -> bool;
    fn is_stream_oriented(&self) -> bool;
    fn insert_line_class_in_proper_place(&mut self, x: Rc<SpecifierClass>);
}
impl SpecifierStackExt for SpecifierStack {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;

        if !self.addrtype.is_stdio() {
            return None;
        }

        let mut sus: StdioUsageStatus = IsItself;

        for overlay in self.overlays.iter().rev() {
            if overlay.is_reuser() {
                sus = WithReuser;
            } else if sus == IsItself {
                sus = Indirectly;
            }
        }
        sus
    }
    fn reuser_count(&self) -> usize {
        let mut c = 0;
        for overlay in &self.overlays {
            if overlay.is_reuser() {
                c += 1;
            }
        }
        c
    }
    fn contains(&self, t: &'static str) -> bool {
        for overlay in &self.overlays {
            if overlay.get_name() == t {
                return true;
            }
        }
        self.addrtype.get_name() == t
    }
    fn is_multiconnect(&self) -> bool {
        use super::ClassMulticonnectStatus::*;
        match self.addrtype.multiconnect_status() {
            MultiConnect => (),
            SingleConnect => return false,
            MulticonnectnessDependsOnInnerType => unreachable!(),
        }
        for overlay in self.overlays.iter().rev() {
            match overlay.multiconnect_status() {
                MultiConnect => (),
                SingleConnect => return false,
                MulticonnectnessDependsOnInnerType => (),
            }
        }
        true
    }
    fn is_stream_oriented(&self) -> bool {
        use super::ClassMessageBoundaryStatus::*;
        let mut q = match self.addrtype.message_boundary_status() {
            StreamOriented => true,
            MessageOriented => false,
            MessageBoundaryStatusDependsOnInnerType => unreachable!(),
        };
        for overlay in self.overlays.iter().rev() {
            match overlay.message_boundary_status() {
                StreamOriented => q = true,
                MessageOriented => q = false,
                MessageBoundaryStatusDependsOnInnerType => (),
            }
        }
        q
    }
    fn insert_line_class_in_proper_place(&mut self, x: Rc<SpecifierClass>) {
        use super::ClassMessageBoundaryStatus::*;
        let mut insert_idx = 0;
        for overlay in &self.overlays {
            match overlay.message_boundary_status() {
                StreamOriented => break,
                MessageOriented => break,
                MessageBoundaryStatusDependsOnInnerType => insert_idx += 1,
            }
        }
        self.overlays.insert(insert_idx, x);
    }
}

impl WebsocatConfiguration2 {
    pub fn inetd_mode(&self) -> bool {
        self.contains_class("InetdClass")
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    #[cfg_attr(feature="cargo-clippy", allow(nonminimal_bool))]
    pub fn websocket_used(&self) -> bool {
        false 
        || self.contains_class("WsConnectClass")
        || self.contains_class("WsClientClass")
        || self.contains_class("WsClientSecureClass")
        || self.contains_class("WsServerClass")
    }

    pub fn contains_class(&self, x: &'static str) -> bool {
        self.s1.contains(x) || self.s2.contains(x)
    }

    pub fn get_exec_parameter(&self) -> Option<&str> {
        if self.s1.addrtype.get_name() == "ExecClass" {
            return Some(self.s1.addr.as_str());
        }
        if self.s2.addrtype.get_name() == "ExecClass" {
            return Some(self.s2.addr.as_str());
        }
        None
    }

    pub fn lint_and_fixup<F>(&mut self, on_warning: &F) -> super::Result<()>
    where
        F: for<'a> Fn(&'a str) -> () + 'static,
    {
        let mut reuser_has_been_inserted = false;
        let multiconnect = !self.opts.oneshot && self.s1.is_multiconnect();
        use self::StdioUsageStatus::{Indirectly, IsItself, None, WithReuser};
        match (self.s1.stdio_usage_status(), self.s2.stdio_usage_status()) {
            (_, None) => (),
            (None, WithReuser) => (),
            (None, IsItself) | (None, Indirectly) => {
                if multiconnect {
                    self.s2.overlays.insert(
                        0,
                        Rc::new(super::broadcast_reuse_peer::BroadcastReuserClass),
                    );
                    reuser_has_been_inserted = true;
                }
            }
            (IsItself, IsItself) => {
                info!("Special mode, expection from usual one-stdio rule. Acting like `cat(1)`");
                self.s2 = SpecifierStack::from_str("mirror:")?;
                if self.opts.unidirectional ^ self.opts.unidirectional_reverse {
                    self.opts.unidirectional = false;
                    self.opts.unidirectional_reverse = false;
                }
                return Ok(());
            }
            (_, _) => {
                Err("Too many usages of stdin/stdout. Specify it either on left or right address, not on both.")?;
            }
        }

        if self.s1.reuser_count() + self.s2.reuser_count() > 1 {
            if reuser_has_been_inserted {
                error!("The reuser you specified conflicts with automatically inserted reuser based on usage of stdin/stdout in multiconnect mode.");
            }
            Err("Too many usages of connection reuser. Please limit to only one instance.")?;
        }

        if !self.opts.no_auto_linemode && self.opts.websocket_text_mode {
            match (self.s1.is_stream_oriented(), self.s2.is_stream_oriented()) {
                (false, false) => {}
                (true, true) => {}
                (true, false) => {
                    info!("Auto-inserting the line mode");
                    self.s1.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Line2MessageClass,
                    ));
                    self.s2.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Message2LineClass,
                    ));
                }
                (false, true) => {
                    info!("Auto-inserting the line mode");
                    self.s2.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Line2MessageClass,
                    ));
                    self.s1.insert_line_class_in_proper_place(Rc::new(
                        super::line_peer::Message2LineClass,
                    ));
                }
            }
        }

        if !self.opts.oneshot && self.s2.is_multiconnect() && !self.s1.is_multiconnect() {
            on_warning("You have specified a listener on the right (as the second positional argument) instead of on the left. It will only serve one connection.\nChange arguments order to enable multiple parallel connections or use --oneshot argument to make single connection explicit.");
        }

        if multiconnect
            && (self.s2.addrtype.get_name() == "WriteFileClass"
                || self.s2.addrtype.get_name() == "AppendFileClass")
            && self.s2.reuser_count() == 0
        {
            info!("Auto-inserting the reuser");
            self.s2
                .overlays
                .push(Rc::new(super::primitive_reuse_peer::ReuserClass));
        }

        if self.s1.addrtype.get_name() == "ExecClass" && self.s2.addrtype.get_name() == "ExecClass"
        {
            Err("Can't use exec: more than one time. Replace one of them with sh-c: or cmd:.")?;
        }

        if let Some(x) = self.get_exec_parameter() {
            if self.opts.exec_args.is_empty() && x.contains(' ') {
                on_warning("Warning: you specified exec: without the corresponding --exec-args at the end of command line. Unlike in cmd: or sh-c:, spaces inside exec:'s direct parameter are interpreted as part of program name, not as separator.");
            }
        }

        // TODO: UDP connect oneshot mode

        // TODO: tests for the linter
        Ok(())
    }
}
