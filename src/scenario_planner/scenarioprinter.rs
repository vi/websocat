use base64::Engine as _;

pub struct ScenarioPrinter {
    out: String,
    indent: usize,
}

impl Default for ScenarioPrinter {
    fn default() -> Self {
        Self::new()
    }
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

pub struct StrLit<T: std::fmt::Display>(pub T);
impl<T: std::fmt::Display> std::fmt::Display for StrLit<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tmp = format!("{}", self.0);
        if tmp.contains('"') || tmp.contains('\\') || tmp.contains('\n') {
            write!(
                f,
                "b64str(\"{}\")",
                base64::prelude::BASE64_STANDARD.encode(tmp)
            )
        } else {
            write!(f, "\"{}\"", &tmp)
        }
    }
}
