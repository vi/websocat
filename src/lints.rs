use super::{WebsocatConfiguration2, SpecifierStack, SpecifierClass};
use std::rc::Rc;


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
}
impl SpecifierStackExt for SpecifierStack {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;
        
        if ! self.addrtype.is_stdio() {
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
        for overlay in self.overlays.iter() {
            if overlay.is_reuser() {
                c += 1;
            }
        }
        c
    }
    fn contains(&self, t: &'static str) -> bool {
        for overlay in self.overlays.iter() {
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
        return true;
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
                StreamOriented => q=true,
                MessageOriented => q=false,
                MessageBoundaryStatusDependsOnInnerType => (),
            }
        }
        return q;
    }
}

impl WebsocatConfiguration2 {
    pub fn lint_and_fixup<F>(&mut self, _on_warning: Rc<F>) -> super::Result<()> 
        where F: for<'a> Fn(&'a str) -> () + 'static
    {
        let mut reuser_has_been_inserted = false;
        use self::StdioUsageStatus::{IsItself, WithReuser, Indirectly, None};
        match (self.s1.stdio_usage_status(), self.s2.stdio_usage_status()) {
            (_, None) => (),
            (None, WithReuser) => (),
            (None, IsItself) | (None, Indirectly) => {
                if !self.opts.oneshot && self.s1.is_multiconnect() {
                    self.s2.overlays.insert(0,
                        Rc::new(super::broadcast_reuse_peer::BroadcastReuserClass));
                    reuser_has_been_inserted = true;
                }
            },
            (IsItself, IsItself) => {
                info!("Special mode, expection from usual one-stdio rule. Acting like `cat(1)`");
                self.s2 = SpecifierStack::from_str("mirror:")?;
                if self.opts.unidirectional ^ self.opts.unidirectional_reverse {
                    self.opts.unidirectional = false;
                    self.opts.unidirectional_reverse = false;
                }
                return Ok(());
            },
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
                (false,false) => {},
                (true,true) => {},
                (true,false) => {
                    info!("Auto-inserting the line mode");
                    self.s1.overlays.insert(0,
                        Rc::new(super::line_peer::Line2MessageClass));
                    self.s2.overlays.insert(0,
                        Rc::new(super::line_peer::Message2LineClass));
                },
                (false, true) => {
                    info!("Auto-inserting the line mode");
                    self.s2.overlays.insert(0,
                        Rc::new(super::line_peer::Line2MessageClass));
                    self.s1.overlays.insert(0,
                        Rc::new(super::line_peer::Message2LineClass));
                }
            }
        }
        
        // TODO: listener at right
        // TODO: UDP connect oneshot mode
        // TODO: early fail for reuse:
        // TODO: writefile and reuse:
        // TODO: warn about reuse: for non-stdio
        // TODO: multiple exec:s
        // TODO: exec: without --exec-args
        
        Ok(())
    }
}

