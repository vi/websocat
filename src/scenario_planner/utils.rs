use std::{collections::HashMap, ffi::OsStr};
use clap_lex::OsStrExt;

pub struct IdentifierGenerator {
    pub varnames: HashMap<&'static str, usize>,
}

impl IdentifierGenerator {
    pub fn new() -> Self {
        IdentifierGenerator {
            varnames: HashMap::with_capacity(8),
        }
    }
    pub fn getnewvarname(&mut self, prefix: &'static str) -> String {
        let e = self.varnames.entry(prefix).or_default();
        *e += 1;
        return format!("{prefix}{}", *e);
    }
}

pub trait StripPrefixMany {
    fn strip_prefix_many<'a>(&'a self, prefixes: &'static [&'static str]) -> Option<&'a OsStr>;
}
impl StripPrefixMany for OsStr {
    fn strip_prefix_many<'a>(&'a self, prefixes: &'static [&'static str]) -> Option<&'a OsStr> {
        for p in prefixes {
            if let Some(x) = self.strip_prefix(p) {
                return Some(x)
            }
        }
        None
    }
}
