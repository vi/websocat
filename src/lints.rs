use super::{connection_reuse_peer, Specifier, SpecifierType, WebsocatConfiguration};
use std::rc::Rc;
use super::line_peer;

/// Diagnostics for specifiers and options combinations
#[derive(PartialEq, Eq)]
pub enum ConfigurationConcern {
    StdinToStdout,
    StdioConflict,
    NeedsStdioReuser,
    NeedsStdioReuser2,
    MultipleReusers,
    DegenerateMode,
}

#[derive(PartialEq, Eq,Clone,Copy)]
pub enum AutoInstallLinemodeConcern {
    NoWebsocket,
    MultipleWebsocket,
    AlreadyLine,
}

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

trait SpecifierExt {
    fn stdio_usage_status(&self) -> StdioUsageStatus;
    fn reuser_count(&self) -> usize;
    fn contains(&self, t:SpecifierType) -> bool;
}

impl<T: Specifier> SpecifierExt for T {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;
        let mut sus: StdioUsageStatus = None;

        for i in self.get_info().collect().iter().rev() {
            match i.typ {
                SpecifierType::Stdio => {
                    sus = IsItself;
                }
                SpecifierType::Reuser => {
                    if sus >= Indirectly {
                        sus = WithReuser;
                    }
                }
                _ => {
                    if sus == IsItself {
                        sus = Indirectly;
                    }
                }
            }
        }
        sus
    }
    fn reuser_count(&self) -> usize {
        let mut count = 0;

        for i in self.get_info().collect() {
            if i.typ == SpecifierType::Reuser {
                count += 1;
            }
        }
        count
    }
    
    fn contains(&self, t:SpecifierType) -> bool {
        for i in self.get_info().collect() {
            if i.typ == t {
                return true;
            }
        }
        false
    }
}

impl WebsocatConfiguration {
    pub fn get_concern(&self) -> Option<ConfigurationConcern> {
        use self::ConfigurationConcern::*;
        use self::StdioUsageStatus::{IsItself, WithReuser};

        if self.s1.stdio_usage_status() == IsItself && self.s2.stdio_usage_status() == IsItself {
            if self.opts.unidirectional && self.opts.unidirectional_reverse {
                return Some(DegenerateMode);
            }
            return Some(StdinToStdout);
        }

        if self.s1.stdio_usage_status() >= WithReuser && self.s2.stdio_usage_status() >= WithReuser
        {
            return Some(StdioConflict);
        }

        if self.s1.is_multiconnect() && self.s2.stdio_usage_status() > WithReuser
            && !self.opts.oneshot
        {
            if !self.opts.unidirectional {
                return Some(NeedsStdioReuser);
            } else {
                return Some(NeedsStdioReuser2);
            }
        }

        if self.s1.reuser_count() + self.s2.reuser_count() > 1 {
            return Some(MultipleReusers);
        }

        // TODO: listener at right
        // TODO: UDP connect oneshot mode
        // TODO: early fail for reuse:
        // TODO: writefile and reuse:
        // TODO: warn about reuse: for non-stdio
        // TODO: multiple exec:s
        // TODO: exec: without --exec-args

        None
    }

    pub fn auto_install_reuser(self) -> Self {
        let WebsocatConfiguration { opts, s1, s2 } = self;
        WebsocatConfiguration {
            opts,
            s1,
            s2: Rc::new(connection_reuse_peer::Reuser(s2)),
        }
    }
    
    pub fn auto_install_linemode(self) -> Result<Self, (AutoInstallLinemodeConcern,Self)> {
        use self::AutoInstallLinemodeConcern::*;
        use SpecifierType::{Line,WebSocket};
        if self.s1.contains(Line) { return Err((AlreadyLine,self)) }
        if self.s2.contains(Line) { return Err((AlreadyLine,self)) }
        if self.s1.contains(WebSocket) {
            if self.s2.contains(WebSocket) {
                Err((MultipleWebsocket, self))
            } else {
                let WebsocatConfiguration { opts, s1, s2 } = self;
                Ok(WebsocatConfiguration {
                    opts,
                    s1: Rc::new(line_peer::Message2Line(s1)),
                    s2: Rc::new(line_peer::Line2Message(s2)),
                })
            }
        } else {
            if self.s2.contains(WebSocket) {
                let WebsocatConfiguration { opts, s1, s2 } = self;
                Ok(WebsocatConfiguration {
                    opts,
                    s1: Rc::new(line_peer::Line2Message(s1)),
                    s2: Rc::new(line_peer::Message2Line(s2)),
                })
            } else {
                Err((NoWebsocket, self))
            }
        }
    }
}
