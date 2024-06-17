use std::collections::HashMap;

pub struct IdentifierGenerator {
    pub varnames : HashMap<&'static str, usize>,
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
