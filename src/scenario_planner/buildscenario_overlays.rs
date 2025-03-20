use super::{
    scenarioprinter::StrLit,
    types::{Overlay, ScenarioPrintingEnvironment},
};

impl Overlay {
    pub(super) fn begin_print(
        &self,
        env: &mut ScenarioPrintingEnvironment<'_>,
        inner_var: &str,
    ) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.begin_print_ws(env, inner_var),
            Overlay::StreamChunks => {
                let varnam = env.vars.getnewvarname("chunks");
                env.printer
                    .print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
            Overlay::LineChunks => {
                let varnam = env.vars.getnewvarname("chunks");
                let mut oo = String::new();
                if let Some(ref x) = env.opts.separator {
                    oo.push_str(&format!("separator: {x},"));
                }
                if env.opts.null_terminated {
                    oo.push_str("separator: 0,");
                }
                if let Some(ref x) = env.opts.separator_n {
                    oo.push_str(&format!("separator_n: {x},"));
                }
                if !env.opts.separator_inhibit_substitution {
                    oo.push_str("substitute: 32,");
                }
                env.printer.print_line(&format!(
                    "let {varnam} = line_chunks(#{{{oo}}}, {inner_var});"
                ));
                Ok(varnam)
            }
            Overlay::LengthPrefixedChunks => {
                let varnam = env.vars.getnewvarname("chunks");
                let mut oo = String::new();

                let nbytes = env.opts.lengthprefixed_nbytes;
                if !(1..=8).contains(&nbytes) {
                    anyhow::bail!("`--lengthprefixed-nbytes` must be from 1 to 8");
                }
                let mut highest_unused_bit: u64 = 1 << (8 * nbytes - 1);

                if env.opts.lengthprefixed_little_endian {
                    oo.push_str("little_endian: true,");
                }
                if env.opts.lengthprefixed_skip_read_direction {
                    oo.push_str("skip_read_direction: true,");
                }
                if env.opts.lengthprefixed_skip_write_direction {
                    oo.push_str("skip_write_direction: true,");
                }
                if env.opts.lengthprefixed_continuations {
                    oo.push_str(&format!("continuations: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }
                if env.opts.lengthprefixed_include_control {
                    oo.push_str(&format!("controls: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }
                if env.opts.lengthprefixed_tag_text {
                    oo.push_str(&format!("tag_text: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }

                let length_mask: u64 = ((highest_unused_bit - 1) << 1) + 1;

                let mut max_message_size = env.opts.lengthprefixed_max_message_size;
                if max_message_size as u64 > length_mask {
                    max_message_size = length_mask as usize;
                }

                oo.push_str(&format!("max_message_size: {max_message_size},"));
                oo.push_str(&format!("nbytes: {nbytes},"));
                oo.push_str(&format!("length_mask: {length_mask},"));

                env.printer.print_line(&format!(
                    "let {varnam} = length_prefixed_chunks(#{{{oo}}}, {inner_var});"
                ));
                Ok(varnam)
            }
            Overlay::TlsClient {
                domain,
                varname_for_connector,
            } => {
                assert!(!varname_for_connector.is_empty());
                let outer_var = env.vars.getnewvarname("tls");

                env.printer.print_line(&format!("tls_client(#{{domain: {dom}}}, {varname_for_connector}, {inner_var}, |{outer_var}| {{", dom=StrLit(domain)));
                env.printer.increase_indent();

                Ok(outer_var)
            }
            Overlay::Log { datagram_mode } => {
                let varnam = env.vars.getnewvarname("log");

                let funcname = if *datagram_mode {
                    "datagram_logger"
                } else {
                    "stream_logger"
                };

                let maybe_loghex = if env.opts.log_hex { "hex: true," } else { "" };

                let maybe_log_omit_content = if env.opts.log_omit_content {
                    "omit_content: true,"
                } else {
                    ""
                };

                let maybe_log_verbose = if env.opts.log_verbose {
                    "verbose: true,"
                } else {
                    ""
                };

                let maybe_include_timestamps = if env.opts.log_timestamps {
                    "include_timestamps: true,"
                } else {
                    ""
                };

                env.printer.print_line(&format!("let {varnam} = {funcname}(#{{{maybe_loghex}{maybe_log_omit_content}{maybe_log_verbose}{maybe_include_timestamps}}}, {inner_var});"));
                Ok(varnam)
            }
            Overlay::ReadChunkLimiter => {
                let n = env.opts.read_buffer_limit.unwrap_or(1);
                env.printer.print_line(&format!("put_read_part({inner_var}, read_chunk_limiter(take_read_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteChunkLimiter => {
                let n = env.opts.write_buffer_limit.unwrap_or(1);
                env.printer.print_line(&format!("put_write_part({inner_var}, write_chunk_limiter(take_write_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteBuffer => {
                env.printer.print_line(&format!(
                    "put_write_part({inner_var}, write_buffer(take_write_part({inner_var}), 8192));"
                ));
                Ok(inner_var.to_owned())
            }
            Overlay::SimpleReuser => {
                panic!("SimpleReuser is not supposed to emit scenario chunk as an overlay")
            }
            Overlay::WriteSplitoff => {
                panic!("WriteSplitoff should be converted into an endpoint");
            }
        }
    }
    pub(super) fn end_print(&self, env: &mut ScenarioPrintingEnvironment<'_>) {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.end_print_ws(env),
            Overlay::StreamChunks => (),
            Overlay::LineChunks => (),
            Overlay::LengthPrefixedChunks => (),
            Overlay::TlsClient { .. } => {
                env.printer.decrease_indent();
                env.printer.print_line("})");
            }
            Overlay::Log { .. } => (),
            Overlay::ReadChunkLimiter => (),
            Overlay::WriteChunkLimiter => (),
            Overlay::WriteBuffer => (),
            Overlay::SimpleReuser => {
                panic!("SimpleReuser is not supposed to emit scenario chunk as an overlay")
            }
            Overlay::WriteSplitoff => {
                panic!("WriteSplitoff should be converted into an endpoint");
            }
        }
    }
}
