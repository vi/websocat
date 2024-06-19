pub struct ScenarioPrinter {
    out: String,
    indent: usize,
}

impl ScenarioPrinter {
    pub fn new() -> ScenarioPrinter {
        ScenarioPrinter {
            out: String::with_capacity(1024),
            indent: 0,
        }
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
