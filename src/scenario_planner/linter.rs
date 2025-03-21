use super::types::{Endpoint, SocketType, WebsocatInvocation};

#[derive(strum_macros::Display)]
pub enum Lint {
    #[strum(
        to_string = "--oneshot may fail to properly exit process when stdin is used (https://github.com/tokio-rs/tokio/issues/2466). If only stdout is needed, add -u; or add --exit-after-one-session to force exit after serving the connection."
    )]
    StdoutOneshotWithoutExit,
    #[strum(
        to_string = "--global-timeout-ms may fail to properly exit process when stdin is used (https://github.com/tokio-rs/tokio/issues/2466). You may want to also add the --global-timeout-force-exit option."
    )]
    StdoutGlobalTimeoutWithoutExit,
    #[strum(
        to_string = "Listening specifier should be the first specifier (at the left side) in command line. It would server only one connection if found at the right side. Use --oneshot if this is intended"
    )]
    ListenerAtTheWrongSide,
    #[strum(
        to_string = "You have specified separator-related option (--separator or --separator-n or --null-terminated), but Websocat is operating in bytestream-oriented mode where separators are not used. Consider `-t` option or `lines:` overlay."
    )]
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

        if self.session_socket_type() == SocketType::ByteStream
            && (self.opts.separator.is_some()
                || self.opts.separator_n.is_some()
                || self.opts.null_terminated)
        {
            ret.push(Lint::UnusedSeparatorOption)
        }

        ret
    }
}
