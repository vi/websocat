
use super::{Specifier,SpecifierType,WebsocatConfiguration,connection_reuse_peer};
use std::rc::Rc;

/// Diagnostics for specifiers and options combinations
#[derive(PartialEq,Eq)]
pub enum ConfigurationConcern {
    StdinToStdout,
    StdioConflict,
    NeedsStdioReuser,
    MultipleReusers,
    DegenerateMode,
}


#[derive(Ord,PartialOrd,Eq,PartialEq,Copy,Clone)]
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
}

impl<T:Specifier> SpecifierExt for T {
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;
        let mut sus : StdioUsageStatus = None;
        
        for i in self.get_info().collect().iter().rev() {
            match i.typ {
                SpecifierType::Stdio => {
                    sus = IsItself;
                },
                SpecifierType::Reuser => {
                    if sus >= Indirectly {
                        sus = WithReuser;
                    }
                },
                SpecifierType::Other => {
                    if sus == IsItself {
                        sus = Indirectly;
                    }
                },
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
}


impl WebsocatConfiguration {    
    pub fn get_concern(&self) -> Option<ConfigurationConcern> {
        use self::ConfigurationConcern::*;
        use self::StdioUsageStatus::{IsItself,WithReuser};
    
        if self.s1.stdio_usage_status() == IsItself && self.s2.stdio_usage_status() == IsItself {
            if self.opts.unidirectional && self.opts.unidirectional_reverse {
                return Some(DegenerateMode);
            }
            return Some(StdinToStdout);
        }
        
        if self.s1.stdio_usage_status() >= WithReuser && self.s2.stdio_usage_status() >= WithReuser {
            return Some(StdioConflict);
        }
        
        if self.s1.is_multiconnect() && self.s2.stdio_usage_status() > WithReuser {
            return Some(NeedsStdioReuser);
        }
        
        if self.s1.reuser_count() + self.s2.reuser_count() > 1 {
            return Some(MultipleReusers);
        }
        
        // TODO: listener at right
        // TODO: UDP connect oneshot mode
        // TODO: early fail for reuse:
        // TODO: writefile and reuse:
        
        None
    }
    
    pub fn auto_install_reuser(self) -> Self {
        let WebsocatConfiguration { opts, s1, s2 } = self;
        WebsocatConfiguration { opts, s1, s2: Rc::new(connection_reuse_peer::Reuser(s2)) }
    }
}
