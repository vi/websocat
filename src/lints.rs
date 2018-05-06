
use super::{Specifier,SpecifierType,WebsocatConfiguration,connection_reuse_peer};
use std::rc::Rc;
use std::any::Any;

/// Diagnostics for specifiers and options combinations
#[derive(PartialEq,Eq)]
pub enum ConfigurationConcern {
    StdinToStdout,
    StdioConflict,
    NeedsStdioReuser,
    MultipleReusers,
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
    fn types_path(&self) -> Vec<SpecifierType>;
    fn stdio_usage_status(&self) -> StdioUsageStatus;
    fn reuser_count(&self) -> usize;
}

impl<T:Specifier> SpecifierExt for T {
    fn types_path(&self) -> Vec<SpecifierType> {
        let mut rr = vec![];
        for x in self.visit_hierarchy(Rc::new(|y:&Specifier| {
            let c : SpecifierType = y.get_type();
            Box::new(c) as Box<Any>
        })) {
            let c : SpecifierType = *x.downcast().unwrap();
            rr.push(c);
        }
        return rr;
    }
    fn stdio_usage_status(&self) -> StdioUsageStatus {
        use self::StdioUsageStatus::*;
        let mut sus : StdioUsageStatus = None;
        
        for i in self.types_path().iter().rev() {
            match i {
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
        self.types_path().iter().map(
            |x|if *x == SpecifierType::Reuser { 1 } else { 0 }
        ).sum()
    }
}


impl WebsocatConfiguration {    
    pub fn get_concern(&self) -> Option<ConfigurationConcern> {
        use self::ConfigurationConcern::*;
        use self::StdioUsageStatus::{IsItself,WithReuser};
    
        if self.s1.stdio_usage_status() == IsItself && self.s2.stdio_usage_status() == IsItself {
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
        None
    }
    
    pub fn auto_install_reuser(self) -> Self {
        let WebsocatConfiguration { opts, s1, s2 } = self;
        WebsocatConfiguration { opts, s1, s2: Rc::new(connection_reuse_peer::Reuser(s2)) }
    }
}
