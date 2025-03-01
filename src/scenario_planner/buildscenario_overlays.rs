use crate::cli::WebsocatArgs;

use super::{
    scenarioprinter::{ScenarioPrinter, StrLit},
    types::Overlay,
    utils::IdentifierGenerator,
};

impl Overlay {
    pub(super) fn begin_print(
        &self,
        printer: &mut ScenarioPrinter,
        inner_var: &str,
        vars: &mut IdentifierGenerator,
        opts: &WebsocatArgs,
    ) -> anyhow::Result<String> {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.begin_print_ws(printer, inner_var, vars, opts),
            Overlay::StreamChunks => {
                let varnam = vars.getnewvarname("chunks");
                printer.print_line(&format!("let {varnam} = stream_chunks({inner_var});"));
                Ok(varnam)
            }
            Overlay::LineChunks => {
                let varnam = vars.getnewvarname("chunks");
                let mut oo = String::new();
                if let Some(ref x) = opts.separator {
                    oo.push_str(&format!("separator: {x},"));
                }
                if let Some(ref x) = opts.separator_n {
                    oo.push_str(&format!("separator_n: {x},"));
                }
                if !opts.separator_inhibit_substitution {
                    oo.push_str("substitute: 32,");
                }
                printer.print_line(&format!(
                    "let {varnam} = line_chunks(#{{{oo}}}, {inner_var});"
                ));
                Ok(varnam)
            }
            Overlay::LengthPrefixedChunks => {
                let varnam = vars.getnewvarname("chunks");
                let mut oo = String::new();

                let nbytes = opts.lengthprefixed_nbytes;
                if !(1..=8).contains(&nbytes) {
                    anyhow::bail!("`--lengthprefixed-nbytes` must be from 1 to 8");
                }
                let mut highest_unused_bit: u64 = 1 << (8 * nbytes - 1);

                if opts.lengthprefixed_little_endian {
                    oo.push_str("little_endian: true,");
                }
                if opts.lengthprefixed_skip_read_direction {
                    oo.push_str("skip_read_direction: true,");
                }
                if opts.lengthprefixed_skip_write_direction {
                    oo.push_str("skip_write_direction: true,");
                }
                if opts.lengthprefixed_continuations {
                    oo.push_str(&format!("continuations: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }
                if opts.lengthprefixed_include_control {
                    oo.push_str(&format!("controls: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }
                if opts.lengthprefixed_tag_text {
                    oo.push_str(&format!("tag_text: {highest_unused_bit},"));
                    highest_unused_bit >>= 1;
                }

                let length_mask: u64 = ((highest_unused_bit - 1) << 1) + 1;

                let mut max_message_size = opts.lengthprefixed_max_message_size;
                if max_message_size as u64 > length_mask {
                    max_message_size = length_mask as usize;
                }

                oo.push_str(&format!("max_message_size: {max_message_size},"));
                oo.push_str(&format!("nbytes: {nbytes},"));
                oo.push_str(&format!("length_mask: {length_mask},"));

                printer.print_line(&format!(
                    "let {varnam} = length_prefixed_chunks(#{{{oo}}}, {inner_var});"
                ));
                Ok(varnam)
            }
            Overlay::TlsClient {
                domain,
                varname_for_connector,
            } => {
                assert!(!varname_for_connector.is_empty());
                let outer_var = vars.getnewvarname("tls");

                printer.print_line(&format!("tls_client(#{{domain: {dom}}}, {varname_for_connector}, {inner_var}, |{outer_var}| {{", dom=StrLit(domain)));
                printer.increase_indent();

                Ok(outer_var)
            }
            Overlay::Log { datagram_mode } => {
                let varnam = vars.getnewvarname("log");

                let funcname = if *datagram_mode {
                    "datagram_logger"
                } else {
                    "stream_logger"
                };

                let maybe_loghex = if opts.log_hex { "hex: true," } else { "" };

                let maybe_log_omit_content = if opts.log_omit_content {
                    "omit_content: true,"
                } else {
                    ""
                };

                let maybe_log_verbose = if opts.log_verbose {
                    "verbose: true,"
                } else {
                    ""
                };

                let maybe_include_timestamps = if opts.log_timestamps {
                    "include_timestamps: true,"
                } else {
                    ""
                };

                printer.print_line(&format!("let {varnam} = {funcname}(#{{{maybe_loghex}{maybe_log_omit_content}{maybe_log_verbose}{maybe_include_timestamps}}}, {inner_var});"));
                Ok(varnam)
            }
            Overlay::ReadChunkLimiter => {
                let n = opts.read_buffer_limit.unwrap_or(1);
                printer.print_line(&format!("put_read_part({inner_var}, read_chunk_limiter(take_read_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteChunkLimiter => {
                let n = opts.write_buffer_limit.unwrap_or(1);
                printer.print_line(&format!("put_write_part({inner_var}, write_chunk_limiter(take_write_part({inner_var}), {n}));"));
                Ok(inner_var.to_owned())
            }
            Overlay::WriteBuffer => {
                printer.print_line(&format!(
                    "put_write_part({inner_var}, write_buffer(take_write_part({inner_var}), 8192));"
                ));
                Ok(inner_var.to_owned())
            }
        }
    }
    pub(super) fn end_print(&self, printer: &mut ScenarioPrinter) {
        match self {
            Overlay::WsUpgrade { .. }
            | Overlay::WsFramer { .. }
            | Overlay::WsClient
            | Overlay::WsServer
            | Overlay::WsAccept { .. } => self.end_print_ws(printer),
            Overlay::StreamChunks => (),
            Overlay::LineChunks => (),
            Overlay::LengthPrefixedChunks => (),
            Overlay::TlsClient { .. } => {
                printer.decrease_indent();
                printer.print_line("})");
            }
            Overlay::Log { .. } => (),
            Overlay::ReadChunkLimiter => (),
            Overlay::WriteChunkLimiter => (),
            Overlay::WriteBuffer => (),
        }
    }
}
