use super::{Result, Specifier, SpecifierClass};
use std::rc::Rc;

pub fn spec(s: &str) -> Result<Rc<Specifier>> {
    Specifier::from_str(s)
}

impl Specifier {
    fn from_str(s: &str) -> Result<Rc<Specifier>> {
        #[cfg(not(feature="ssl"))] {
            if s.starts_with("wss://") {
                Err("SSL is not compiled in. Use ws:// or get/make another Websocat build.")?
            }
        }
    
        #[cfg(not(any(target_os = "linux", target_os = "android")))] {
            if s.starts_with("abstract") {
                warn!("Abstract-namespaced UNIX sockets are unlikely to be supported here");
            }
        }
    
        macro_rules! my {
            ($x:expr) => {
                for pre in $x.get_prefixes() {
                    if s.starts_with(pre) {
                        let rest = &s[pre.len()..];
                        return $x.construct(s, rest);
                    }
                }
            }
        }
        list_of_all_specifier_classes!(my);
        
        if s == "inetd-ws:" {
            return spec("ws-l:inetd:")
        } else if s.starts_with("l-ws-unix:") {
            return spec(&format!("ws-l:unix-l:{}", &s[10..]))
        } else if s.starts_with("l-ws-abstract:") {
            return spec(&format!("ws-l:abstract-l::{}", &s[14..]))
        } 
        
        if s.starts_with("open:") {
            return Err("There is no `open:` specifier. Consider `open-async:` or `readfile:` or `writefile:` or `appendfile:`")?;
        } 
        
        #[cfg(not(unix))] {
            if s.starts_with("unix") || s.starts_with("abstract") {
                    Err("`unix*:` or `abstract*:` are not supported in this Websocat build")?
            }
        }
        
        #[cfg(not(feature = "tokio-process"))] {
            if s.starts_with("sh-c:") {
                Err("`sh-c:` is not supported in this Websocat build")?
            } else if s.starts_with("exec:") {
                Err("`exec:` is not supported in this Websocat build")?
            }
        }
        
        error!("Invalid specifier string `{}`. See --long-help for the list of specifiers", s);
        Err("Wrong specifier")?
    }
}
