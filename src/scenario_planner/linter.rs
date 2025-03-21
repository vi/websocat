use super::types::{CopyingType, Endpoint, WebsocatInvocation};

pub enum Lint {
    StdoutOneshotWithoutExit,
    StdoutGlobalTimeoutWithoutExit,
    ListenerAtTheWrongSide,
    UnusedSeparatorOption,
}

impl WebsocatInvocation {
    pub fn lints(&self) -> Vec<Lint> {
        let mut ret = vec![];

        if (matches!(self.stacks.left.innermost, Endpoint::Stdio)
            && !self.opts.unidirectional_reverse)
            || (matches!(self.stacks.right.innermost, Endpoint::Stdio) && !self.opts.unidirectional)
        {
            if self.opts.oneshot && !self.opts.exit_after_one_session {
                ret.push(Lint::StdoutOneshotWithoutExit);
            }

            if self.opts.global_timeout_ms.is_some() && !self.opts.global_timeout_force_exit {
                ret.push(Lint::StdoutGlobalTimeoutWithoutExit);
            }
        }

        if !self.stacks.left.is_multiconn(&self.opts) && self.stacks.right.is_multiconn(&self.opts)
        {
            ret.push(Lint::ListenerAtTheWrongSide);
        }

        ret
    }

    pub fn lints2(&self) -> Vec<Lint> {
        let mut ret = vec![];

        if self.get_copying_type() == CopyingType::ByteStream
            && (self.opts.separator.is_some()
                || self.opts.separator_n.is_some()
                || self.opts.null_terminated)
        {
            ret.push(Lint::UnusedSeparatorOption)
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
            Lint::StdoutGlobalTimeoutWithoutExit => {
                "--global-timeout-ms may fail to properly exit process when stdin is used (https://github.com/tokio-rs/tokio/issues/2466). You may want to also add the --global-timeout-force-exit option.".fmt(f)
            }
            Lint::ListenerAtTheWrongSide => {
                "Listening specifier should be the first specifier (at the left side) in command line. It would server only one connection if found at the right side. Use --oneshot if this is intended".fmt(f)
            }
            Lint::UnusedSeparatorOption => {
                "You have specified separator-related option (--separator or --separator-n or --null-terminated), but Websocat is operating in bytestream-oriented mode where separators are not used. Consider `-t` option or `lines:` overlay.".fmt(f)
            },
        }
    }
}
