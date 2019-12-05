use super::specifier::{Specifier, SpecifierClass, SpecifierNode, SpecifierStack};
use super::Result;
use std::rc::Rc;
use std::str::FromStr;

pub fn spec(s: &str) -> Result<Rc<dyn Specifier>> {
    Specifier::from_stack(&SpecifierStack::from_str(s)?)
}

fn some_checks(s: &str) -> Result<()> {
    #[cfg(not(feature = "ssl"))]
    {
        if s.starts_with("wss://") {
            return Err("SSL is not compiled in. Use ws:// or get/make another Websocat build.\nYou can also try to workaround missing SSL by using ws-c:cmd:socat trick (see some ws-c: example)".into());
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        if s.starts_with("abstract") {
            warn!("Abstract-namespaced UNIX sockets are unlikely to be supported here");
        }
    }

    if s.starts_with("open:") {
        return Err("There is no `open:` address type. Consider `open-async:` or `readfile:` or `writefile:` or `appendfile:`".into());
    }

    #[cfg(not(unix))]
    {
        if s.starts_with("unix") || s.starts_with("abstract") {
            return Err("`unix*:` or `abstract*:` are not supported in this Websocat build".into());
        }
    }

    #[cfg(not(feature = "tokio-process"))]
    {
        if s.starts_with("sh-c:") {
            return Err("`sh-c:` is not supported in this Websocat build".into());
        } else if s.starts_with("exec:") {
            return Err("`exec:` is not supported in this Websocat build".into());
        }
    }

    Ok(())
}

impl FromStr for SpecifierStack {
    type Err = Box<dyn (::std::error::Error)>;
    #[cfg_attr(feature = "cargo-clippy", allow(cyclomatic_complexity))]
    fn from_str(s: &str) -> Result<SpecifierStack> {
        some_checks(s)?;

        let mut s = s.to_string();
        let mut overlays = vec![];
        let addrtype;
        let addr;
        let mut found = false;

        'a: loop {
            macro_rules! my {
                ($x:expr) => {
                    for pre in $x.get_prefixes() {
                        if s.starts_with(pre) {
                            let rest = &s[pre.len()..].to_string();
                            if let Some(a) = $x.alias_info() {
                                s = format!("{}{}", a, rest);
                                continue 'a;
                            } else if $x.is_overlay() {
                                let cls = Rc::new($x) as Rc<dyn SpecifierClass>;
                                overlays.push(SpecifierNode { cls });
                                s = rest.to_string();
                                continue 'a;
                            } else {
                                addr = rest.to_string();
                                let cls = Rc::new($x) as Rc<dyn SpecifierClass>;
                                addrtype = SpecifierNode { cls };
                                #[allow(unused_assignments)]
                                {
                                    found = true;
                                }
                                break 'a;
                            }
                        }
                    }
                };
            }
            list_of_all_specifier_classes!(my);
            if !found {
                if let Some(colon) = s.find(':') {
                    return Err(
                        format!("Unknown address or overlay type of `{}:`", &s[..colon]).into(),
                    );
                } else {
                    return Err(
                        format!("Unknown address or overlay type of `{}`\nMaybe you forgot the `:` character?", s).into(),
                    );
                }
            }
        }

        Ok(SpecifierStack {
            addr,
            addrtype,
            overlays,
        })
    }
}

impl dyn Specifier {
    pub fn from_stack(st: &SpecifierStack) -> Result<Rc<dyn Specifier>> {
        let mut x = st.addrtype.cls.construct(st.addr.as_str())?;
        for overlay in st.overlays.iter().rev() {
            x = overlay.cls.construct_overlay(x)?;
        }
        Ok(x)
    }
}
