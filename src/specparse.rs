use super::{Result, Specifier, SpecifierClass, SpecifierStack};
use std::rc::Rc;

pub fn spec(s: &str) -> Result<Rc<Specifier>> {
    Specifier::from_stack(SpecifierStack::from_str(s)?)
}

impl SpecifierStack {
    fn from_str(mut s: &str) -> Result<SpecifierStack> {
        let mut overlays = vec![];
        let addrtype;
        let addr;
        
        'a: loop {
            macro_rules! my {
                ($x:expr) => {
                    for pre in $x.get_prefixes() {
                        if s.starts_with(pre) {
                            let rest = &s[pre.len()..];
                            if $x.is_overlay() {
                                overlays.push(Rc::new($x) as Rc<SpecifierClass>);
                                s = rest;
                                continue 'a;
                            } else {
                                addr = rest.to_string();
                                addrtype = Rc::new($x) as Rc<SpecifierClass>;
                                break 'a;
                            }
                        }
                    }
                    Err(format!("Unknown address or overlay type of `{}`", s))?;
                };
            }
            list_of_all_specifier_classes!(my);
        }
        
        Ok(SpecifierStack { addr, addrtype, overlays })
    }
}

impl Specifier {
    fn from_stack(st: SpecifierStack) -> Result<Rc<Specifier>> {
        let mut x = st.addrtype.construct("FIXME", st.addr.as_str())?;
        for overlay in st.overlays {
            x = overlay.construct_overlay(x)?;
        }
        Ok(x)
    }

    #[cfg_attr(feature = "cargo-clippy", allow(cyclomatic_complexity))]
    #[allow(dead_code)]
    fn from_str(s: &str) -> Result<Rc<Specifier>> {
        #[cfg(not(feature = "ssl"))]
        {
            if s.starts_with("wss://") {
                Err("SSL is not compiled in. Use ws:// or get/make another Websocat build.\nYou can also try to workaround missing SSL by using ws-c:cmd:socat trick (see some ws-c: example)")?
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
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
            };
        }
        list_of_all_specifier_classes!(my);

        if s == "inetd-ws:" {
            return spec("ws-l:inetd:");
        } else if s.starts_with("l-ws-unix:") {
            return spec(&format!("ws-l:unix-l:{}", &s[10..]));
        } else if s.starts_with("l-ws-abstract:") {
            return spec(&format!("ws-l:abstract-l::{}", &s[14..]));
        }

        if s.starts_with("open:") {
            return Err("There is no `open:` specifier. Consider `open-async:` or `readfile:` or `writefile:` or `appendfile:`")?;
        }

        #[cfg(not(unix))]
        {
            if s.starts_with("unix") || s.starts_with("abstract") {
                Err("`unix*:` or `abstract*:` are not supported in this Websocat build")?
            }
        }

        #[cfg(not(feature = "tokio-process"))]
        {
            if s.starts_with("sh-c:") {
                Err("`sh-c:` is not supported in this Websocat build")?
            } else if s.starts_with("exec:") {
                Err("`exec:` is not supported in this Websocat build")?
            }
        }

        error!(
            "Invalid specifier string `{}`. See --long-help for the list of specifiers",
            s
        );
        Err("Wrong specifier")?
    }
}
