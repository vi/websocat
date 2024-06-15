use std::collections::HashMap;

pub struct ScenarioPrinter {
    out: String,
    varnames : HashMap<&'static str, usize>,
    indent: usize,
}

impl ScenarioPrinter {
    pub fn new() -> ScenarioPrinter {
        ScenarioPrinter { 
            out: String::with_capacity(1024),
            varnames: HashMap::with_capacity(4),
            indent: 0,
        }
    }

    pub fn getnewvarname(&mut self, prefix: &'static str) -> String {
        let e = self.varnames.entry(prefix).or_default();
        *e += 1;
        return format!("{prefix}{}", *e)
    }

    pub fn print_line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }
    pub fn increase_indent(&mut self) {
        self.indent += 1;
    }
    pub fn decrease_indent(&mut self) {
        self.indent -= 1;
    }

    pub fn into_result(self) -> String {
        self.out
    }
}
