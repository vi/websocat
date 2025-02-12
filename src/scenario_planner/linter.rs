use super::types::{Endpoint, WebsocatInvocation};

pub enum Lint {
    StdoutOneshotWithoutExit,
}

impl WebsocatInvocation {
    pub fn lints(&self) -> Vec<Lint> {
        let mut ret = vec![];

        if (matches!(self.left.innermost, Endpoint::Stdio) && !self.opts.unidirectional_reverse)
            || (matches!(self.right.innermost, Endpoint::Stdio) && !self.opts.unidirectional)
        {
            if self.opts.oneshot && !self.opts.exit_after_one_session {
                ret.push(Lint::StdoutOneshotWithoutExit);
            }
        }

        ret
    }
}

impl std::fmt::Display for Lint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lint::StdoutOneshotWithoutExit => {
                "--oneshot may fail to properly exit process when stdin is used (https://github.com/tokio-rs/tokio/issues/2466). If only stdout is needed, add -u; or add --exit-after-one-session to force exit after serving the connection.".fmt(f)
            }
        }
    }
}
