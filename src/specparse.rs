use super::{Result, Specifier, SpecifierClass, SpecifierStack};
use std::rc::Rc;

pub fn spec(s: &str) -> Result<Rc<Specifier>> {
    Specifier::from_stack(SpecifierStack::from_str(s)?)
}

fn some_checks(s:&str) -> Result<()> {
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
    
    if s.starts_with("open:") {
        return Err("There is no `open:` address type. Consider `open-async:` or `readfile:` or `writefile:` or `appendfile:`")?;
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

    Ok(())
}

impl SpecifierStack {
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
                                overlays.push(Rc::new($x) as Rc<SpecifierClass>);
                                s = rest.to_string();
                                continue 'a;
                            } else {
                                addr = rest.to_string();
                                addrtype = Rc::new($x) as Rc<SpecifierClass>;
                                #[allow(unused_assignments)] {
                                    found = true;
                                }
                                break 'a;
                            }
                        }
                    }
                };
            }
            list_of_all_specifier_classes!(my);
            if ! found {
                Err(format!("Unknown address or overlay type of `{}`", s))?;
            }
        }
        
        Ok(SpecifierStack { addr, addrtype, overlays })
    }
}

impl Specifier {
    fn from_stack(st: SpecifierStack) -> Result<Rc<Specifier>> {
        let mut x = st.addrtype.construct(st.addr.as_str())?;
        for overlay in st.overlays {
            x = overlay.construct_overlay(x)?;
        }
        Ok(x)
    }
}
