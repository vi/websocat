use std::{ffi::OsString, net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Clone, Debug)]
pub struct CustomHeader {
    pub name: String,
    pub value: String,
}
impl CustomHeader {
    fn interpret(x: &str) -> Result<CustomHeader, anyhow::Error> {
        let Some((name, value)) = x.split_once(':') else {
            anyhow::bail!("Custom header requires ':' to separate name and value")
        };
        Ok(CustomHeader {
            name: name.trim().to_owned(),
            value: value.trim().to_owned(),
        })
    }
}

#[derive(Parser, Debug)]
/// Tool to connect to WebSocket, listen them and do other network tricks
#[command(version, about)]
#[command(after_help(include_str!("help_addendum.txt")))]
pub struct WebsocatArgs {
    /// Left endpoint (e.g. a WebSocket URL). May be prefixed by one or more overlays.
    pub spec1: OsString,

    /// Right endpoint (or stdout if omitted). May be prefixed by one or more overlays.
    pub spec2: Option<OsString>,

    /// do not execute this Websocat invocation, print equivalent Rhai script instead.
    #[arg(long)]
    pub dump_spec: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    /// In --compose mode it should be the second argument (i.e. just after --compose)
    #[arg(long)]
    pub dump_spec_phase0: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    #[arg(long)]
    pub dump_spec_phase1: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    #[arg(long)]
    pub dump_spec_phase2: bool,

    /// execute specified file as Rhai script (e.g. resulting from --dump-spec option output)
    #[arg(long, short = 'x')]
    pub scenario: bool,

    /// use text mode (one line = one WebSocket text message)
    #[arg(long, short = 't')]
    pub text: bool,

    /// use binary mode (arbitrary byte chunk = one WebSocket binary message)
    #[arg(long, short = 'b')]
    pub binary: bool,

    /// resolve hostnames to IP addresses late (every time when forwarding a connection) instead of one time at the beginning
    #[arg(long)]
    pub late_resolve: bool,

    /// accept invalid domains and root certificates for TLS client connections
    #[arg(long, short = 'k')]
    pub insecure: bool,

    /// manually specify domain for `tls:` overlay or override domain for `wss://` URLs
    #[arg(long)]
    pub tls_domain: Option<String>,

    /// listen for WebSocket connections instead of establishing client WebSocket connection
    #[arg(long, short = 's')]
    pub server: bool,

    /// log more data from `log:` overlay
    #[arg(long)]
    pub log_verbose: bool,

    /// do not log full content of the data from `log:` overlay, just chunk lengths
    #[arg(long)]
    pub log_omit_content: bool,

    /// use hex lines instead of escaped characters for `log:`` overlay.
    #[arg(long)]
    pub log_hex: bool,

    /// Include relative timestamps in log messages
    #[arg(long)]
    pub log_timestamps: bool,

    /// automatically insert `log:` overlay in an appropriate place to debug issues by displaying traffic chunks
    #[arg(long)]
    pub log_traffic: bool,

    /// URI for `ws-c:` overlay.
    #[arg(long)]
    pub ws_c_uri: Option<String>,

    /// parameter for read_chunk_limiter: overlay, defaults to 1
    #[arg(long)]
    pub read_buffer_limit: Option<usize>,

    /// parameter for write_chunk_limiter: overlay, defaults to 1
    #[arg(long)]
    pub write_buffer_limit: Option<usize>,

    /// override byte value that separates stdin-supplied text WebSocket messages
    /// from each other from default '\n'.
    #[arg(long)]
    pub separator: Option<u8>,

    /// require this number of newline (or other) bytes to separate WebSocket messages
    #[arg(long)]
    pub separator_n: Option<usize>,

    /// prevent mangling incoming text WebSocket by replacing `\n`  (or other
    /// separator sequence) with spaces (and trimming leading and trailing separator bytes)
    #[arg(long)]
    pub separator_inhibit_substitution: bool,

    /// Same as setting `--separator` to `0`. Make text mode messages separated by zero byte instead of newline.
    #[arg(long, short = '0')]
    pub null_terminated: bool,

    /// initial target sendto address for `udp-bind:` mode.
    /// If unset, it will try to send to neutral address (unsuccessfully).
    #[arg(long)]
    pub udp_bind_target_addr: Option<SocketAddr>,

    /// only allow incoming datagrams from specified target address for `upd-bind:` mode.
    #[arg(long)]
    pub udp_bind_restrict_to_one_address: bool,

    /// automatically change target address for `udp-bind:` mode based in coming datagrams
    #[arg(long)]
    pub udp_bind_redirect_to_last_seen_address: bool,

    /// turn `udp-bind:` into `udp-connect:` as soon as we receive some datagram.
    /// Implied when `--udp-bind-target-addr` is not specified
    #[arg(long)]
    pub udp_bind_connect_to_first_seen_address: bool,

    /// ignore failed `sendto` calls.
    /// Attempts to send without a configured target address are ignored implicitly.
    #[arg(long)]
    pub udp_bind_inhibit_send_errors: bool,

    /// Client timeout of udp-server: mode
    #[arg(long)]
    pub udp_server_timeout_ms: Option<u64>,

    /// Maximum number of parallel handlers in udp-server: mode
    #[arg(long)]
    pub udp_server_max_clients: Option<usize>,

    /// Size of receive buffer for udp-server: mode.
    /// `-B` is distinct, but can also affect operation.
    #[arg(long)]
    pub udp_server_buffer_size: Option<usize>,

    /// Queue length for udp-server: mode
    #[arg(long)]
    pub udp_server_qlen: Option<usize>,

    /// Delay receiving more datagrams in udp-server: mode instead of dropping them in case of slow handlers
    #[arg(long)]
    pub udp_server_backpressure: bool,

    /// Command line arguments for `exec:` endpoint.
    ///
    /// This option is interpreted specially: it stops processing all other options
    /// uses everything after it as a part of the command line
    #[arg(long, num_args(..), allow_hyphen_values(true))]
    pub exec_args: Vec<OsString>,

    /// Immediately expire `cmd:` or `exec:` endpoints if child process terminates.
    ///
    /// This may discard some data that remained buffered in a pipe.
    #[arg(long)]
    pub exec_monitor_exits: bool,

    /// On Unix, try to set uid to this numeric value for the subprocess
    #[arg(long)]
    pub exec_uid: Option<u32>,

    /// On Unix, try to set uid to this numeric value for the subprocess
    #[arg(long)]
    pub exec_gid: Option<u32>,

    /// Try to change current directory to this value for the subprocess
    #[arg(long)]
    pub exec_chdir: Option<PathBuf>,

    /// On Windows, try to set this numeric process creation flags
    #[arg(long)]
    pub exec_windows_creation_flags: Option<u32>,

    /// On Unix, set first subprocess's argv[0] to this value
    #[arg(long)]
    pub exec_arg0: Option<OsString>,

    /// On Unix, use dup2 and forward sockets directly to child processes (ignoring any overlays) instead of piping though stdin/stdout.
    /// Argument is comma-separated list of file descriptor slots to duplicate the socket into, e.g. `0,1` for stdin and stdout.
    ///
    /// This is a low-level option that is less tested than other things. Expect non-user-friendly error messages if misused.
    #[arg(long, value_delimiter = ',')]
    pub exec_dup2: Option<Vec<i64>>,

    /// When using --exec-dup2, do not set inherited file descriptors to blocking mode
    #[arg(long)]
    pub exec_dup2_keep_nonblocking: bool,

    /// on Unix, When using `--exec-dup2`, do not return to Websocat, instead substitute Websocat process with the given command.
    #[arg(long)]
    pub exec_dup2_execve: bool,

    /// Make dummy nodes also immediately signal hangup.
    #[arg(long)]
    pub dummy_hangup: bool,

    /// Exit the whole process if hangup is detected.
    #[arg(long)]
    pub exit_on_hangup: bool,

    /// Exit the whole process after serving one connection; alternative to to --oneshot.
    #[arg(long)]
    pub exit_after_one_session: bool,

    /// Transfer data only from left to right specifier
    #[arg(long, short = 'u')]
    pub unidirectional: bool,

    /// Transfer data only from right to left specifier
    #[arg(long, short = 'U')]
    pub unidirectional_reverse: bool,

    /// Do not shutdown inactive directions when using `-u` or `-U`.
    #[arg(long)]
    pub unidirectional_late_drop: bool,

    /// Stop transferring data when one of the transfer directions reached EOF.
    #[arg(long, short = 'E')]
    pub exit_on_eof: bool,

    /// Override buffer size for main data transfer session.
    /// Note that some overlays and endpoints may have separate buffers with separately adjustable sizes.
    ///
    /// Message can span multiple over multiple fragments and exceed this buffer size
    #[arg(long, short = 'B')]
    pub buffer_size: Option<usize>,

    /// Do not send WebSocket close message when there is no more data to send there.
    #[arg(long, short = 'n')]
    pub no_close: bool,

    /// Do not flush after each WebSocket frame.
    #[arg(long)]
    pub ws_no_flush: bool,

    /// Shutdown write direction of the underlying socket backing a WebSocket on EOF.
    #[arg(long)]
    pub ws_shutdown_socket_on_eof: bool,

    /// Do not fail WebSocket connections if masked frame arrives instead of unmasked or vice versa.
    #[arg(long)]
    pub ws_ignore_invalid_masks: bool,

    /// Ignore absence or invalid values of `Sec-Websocket-*` things and just continue connecting.
    #[arg(long)]
    pub ws_dont_check_headers: bool,

    /// Do not automatically insert buffering layer after WebSocket if underlying connections does not support `writev`.
    #[arg(long)]
    pub ws_no_auto_buffer: bool,

    /// Skip request or response headers for Websocket upgrade
    #[arg(long)]
    pub ws_omit_headers: bool,

    /// Add this custom header to WebSocket upgrade request when connecting to a Websocket.
    /// Colon separates name and value.
    #[arg(long, short='H', value_parser=CustomHeader::interpret)]
    pub header: Vec<CustomHeader>,

    /// Add this custom header to WebSocket upgrade response when serving a Websocket connection.
    /// Colon separates name and value.
    #[arg(long, value_parser=CustomHeader::interpret)]
    pub server_header: Vec<CustomHeader>,

    /// Specify this Sec-WebSocket-Protocol: header when connecting to a WebSocket.
    #[arg(long)]
    pub protocol: Option<String>,

    /// Use this `Sec-WebSocket-Protocol:` value when serving a Websocket,
    /// and reject incoming connections if the don't specify this protocol.
    #[arg(long)]
    pub server_protocol: Option<String>,

    /// Don't reject incoming connections that fail to specify proper `Sec-WebSocket-Protocol`
    /// header. The header would be omitted from the response in this case.
    #[arg(long)]
    pub server_protocol_lax: bool,

    /// If client specifies Sec-WebSocket-Protocol, choose the first mentioned protocol
    /// and use if for response's Sec-WebSocket-Protocol.
    #[arg(long)]
    pub server_protocol_choose_first: bool,

    /// When listening UNIX sockets, attempt to delete the file first to avoid the failure to bind
    #[arg(long)]
    pub unlink: bool,

    /// When listening UNIX sockets, change socket filesystem permissions to only allow owner connections
    #[arg(long)]
    pub chmod_owner: bool,

    /// When listening UNIX sockets, change socket filesystem permissions to allow owner and group connections
    #[arg(long)]
    pub chmod_group: bool,

    /// When listening UNIX sockets, change socket filesystem permissions to allow connections from everywhere
    #[arg(long)]
    pub chmod_everyone: bool,

    /// Serve only one connection
    #[arg(long)]
    pub oneshot: bool,

    /// Do not display warnings about potential CLI misusage
    #[arg(long)]
    pub no_lints: bool,

    /// Do not automatically transform endpoints and overlays to their appropriate low-level form.
    /// Many things will fail in this mode.
    #[arg(long)]
    pub no_fixups: bool,

    /// Inhibit some optional transformations of specifier stacks, such as auto-inserting of a reuser
    #[arg(long)]
    pub less_fixups: bool,

    /// Maximum size of an outgoing UDP datagram. Incoming datagram size is likely limited by --buffer-size.
    #[arg(long, default_value = "4096")]
    pub udp_max_send_datagram_size: usize,

    /// Maximum size of an outgoing SEQPACKET datagram. Incoming datagram size is likely limited by --buffer-size.
    #[arg(long, default_value = "1048576")]
    pub seqpacket_max_send_datagram_size: usize,

    /// Use specified random seed instead of initialising RNG from OS.
    #[arg(long)]
    pub random_seed: Option<u64>,

    /// Use specified max buffer size for
    #[arg(long, default_value = "1024")]
    pub registry_connect_bufsize: usize,

    /// Use specified buffer size for mirror: endpoint
    #[arg(long, default_value = "1024")]
    pub mirror_bufsize: usize,

    /// Use little-endian framing headers instead of big-endian for `lengthprefixed:` overlay.
    #[arg(long)]
    pub lengthprefixed_little_endian: bool,

    /// Only affect one direction of the `lengthprefixed:` overlay, bypass transformation for the other one.
    #[arg(long)]
    pub lengthprefixed_skip_read_direction: bool,

    /// Only affect one direction of the `lengthprefixed:` overlay, bypass transformation for the other one.
    #[arg(long)]
    pub lengthprefixed_skip_write_direction: bool,

    /// Use this number of length header bytes for `lengthprefixed:` overlay.
    #[arg(long, default_value = "4")]
    pub lengthprefixed_nbytes: usize,

    /// Do not reassemble messages from fragments, stream them as chunks.
    /// Highest bit of the prefix would be set if the message is non-final
    #[arg(long)]
    pub lengthprefixed_continuations: bool,

    /// Maximum size of `lengthprefixed:` message (that needs to be reassembled from fragments)
    ///
    /// Connections would fail when messages exceed this size.
    ///
    /// Ignored if `--lengthprefixed-continuations` is active, but `nbytes`-based limitation can still fail connections.
    #[arg(long, default_value = "1048576")]
    pub lengthprefixed_max_message_size: usize,

    /// Include inline control messages (i.e. WebSocket pings or close frames) as content in `lengthprefixed:`.
    ///
    /// A bit would be set to signify a control message and opcode will be prepended as the first byte.
    ///
    /// When both continuations and controls are enabled, control messages may appear between continued data message chunks.
    /// Control messages can themselves be subject to continuations.
    #[arg(long)]
    pub lengthprefixed_include_control: bool,

    /// Set a bit in the prefix of `lengthprefixed:` frames when the frame denotes a text WebSocket message instead of binary.
    #[arg(long)]
    pub lengthprefixed_tag_text: bool,

    /// Stop automatic replying to WebSocket pings after sending specified number of pongs. May be zero to just disable replying to pongs.
    #[arg(long)]
    pub inhibit_pongs: Option<usize>,

    /// Abort whatever Websocat is doing after specified number of milliseconds, regardless of whether something is connected or not.
    #[arg(long)]
    pub global_timeout_ms: Option<u64>,

    /// Force process exit when global timeout is reached
    #[arg(long)]
    pub global_timeout_force_exit: bool,

    /// Wait for this number of milliseconds before starting endpoints.
    /// Mostly intended for testing Websocat, in combination with --compose mode.
    #[arg(long)]
    pub sleep_ms_before_start: Option<u64>,

    /// Print a line to stdout when a port you requested to be listened is ready to accept connections.
    #[arg(long)]
    pub stdout_announce_listening_ports: bool,

    /// Execute this command line after binding listening port
    ///
    /// Connections are not actually accepted until the command exits (exit code is ignored)
    #[arg(long)]
    pub exec_after_listen: Option<String>,

    /// Append TCP or UDP port number to command line specified in --exec-after-listen
    ///
    /// Makes listening port "0" practical.
    #[arg(long)]
    pub exec_after_listen_append_port: bool,

    /// Show dedicated error message explaining how to migrate Websocat1's --accept-from-fd to the new scheme
    #[arg(long)]
    pub accept_from_fd: bool,

    /// When using `reuse-raw:` (including automatically inserted), do not abort connections on unrecoverable broken messages, instead produce a trimmed message and continue.
    #[arg(long)]
    pub reuser_tolerate_torn_msgs: bool,

    /// Interpret special command line arguments like `&`, `;`, '^', `(` and `)` as separators for composed scenarios mode.
    /// This argument must come first.
    ///
    /// This allows to execute multiple sub-scenarios in one Websocat invocation, sequentially (;), in parallel (&)
    /// or in parallel with early exit (^). You can also use parentheses to combine disparate operations.
    #[arg(long)]
    pub compose: bool,

    /// For `writefile:` endpoint, do not overwrite existing files.
    #[arg(long)]
    pub write_file_no_overwrite: bool,

    /// For `writefile:` endpoint, do not overwrite existing files, instead use other, neighbouring file names.
    #[arg(long)]
    pub write_file_auto_rename: bool,

    /// Add Origin HTTP header to Websocket client request.
    #[arg(long)]
    pub origin: Option<String>,

    /// Add User-Agent HTTP header to Websocket client request.
    #[arg(long)]
    pub ua: Option<String>,

    /// For `random:` endpoint, use smaller and faster RNG instead of secure one
    #[arg(long)]
    pub random_fast: bool,

    /// Specify the write counterpart for `write-splitoff:` overlay.
    /// Expects a specifier like `tcp:127.0.0.1:1234`, like a positional argument.
    #[arg(long)]
    pub write_splitoff: Option<OsString>,

    /// Do not write-shutdown the read part of a `write-splitoff:` overlay.
    #[arg(long)]
    pub write_splitoff_omit_shutdown: bool,

    /// Pass traffic through this socket prior to transfer data from left to right specifiers.
    ///
    /// The filter itself can be any specifier (including with overlays), e.g. `--filter=lines:tcp:127.0.0.1:1234`
    #[arg(long)]
    pub filter: Vec<OsString>,

    /// Pass traffic through this socket prior to transfer data from right to left specifiers
    #[arg(long)]
    pub filter_reverse: Vec<OsString>,

    /// Force using a file descriptor for `async-fd:` even when it cannot be registered for events.
    ///
    /// In case of EWOULDBLOCK Websocat would wait for some short time in a loop.
    ///
    /// In some cases the whole Websocat process may be blocked.
    #[arg(long)]
    pub async_fd_force: bool,

    /// Maximum buffered message size for `defragment:` overlay
    #[arg(long, default_value = "1048576")]
    pub defragment_max_size: usize,

    /// Copy output datagrams also to this specifier; also merge in incoming datagrams from this specifier
    ///
    /// May insert a `tee:` overlay automatically if not specified.
    #[arg(long)]
    pub tee: Vec<OsString>,

    /// Cause `tee:` overlay to fail datagram read or write if any (instead of all) nodes failed the operation.
    #[arg(long)]
    pub tee_propagate_failures: bool,

    /// Cause `tee:` overlay's reading direction to propagate EOF when any of the nodes signaled EOF instead of all of them
    #[arg(long)]
    pub tee_propagate_eof: bool,

    /// When using `tee:`, do not abort reading side of the connections on unrecoverable broken messages, instead produce a trimmed message and continue.
    #[arg(long)]
    pub tee_tolerate_torn_msgs: bool,

    /// Terminate `tee:` specifier when any of the tee nodes signal hangup.
    #[arg(long)]
    pub tee_use_hangups: bool,

    /// Terminate `tee:` specifier when main tee node (the one after `tee:` instead of `--tee`) signals hangup.
    #[arg(long)]
    pub tee_use_first_hangup: bool,

    /// When built with rustls, write encryption infromation to a files based on
    /// `SSLKEYLOGFILE`  envionrment variable to assist decrypting traffic for analysis.
    #[arg(long)]
    pub enable_sslkeylog: bool,
}

/// Subset of command line arguments that describes the whole Websocat operation even when `--compose` sub-scenarios are used.
#[derive(Default)]
pub struct WebsocatGlobalArgs {
    pub random_seed: Option<u64>,
    pub dump_spec: bool,
    pub scenario: Option<PathBuf>,
    pub dump_spec_phase0: bool,
}

impl WebsocatGlobalArgs {
    pub fn hoover(&mut self, args: &WebsocatArgs) -> anyhow::Result<()> {
        if args.random_seed.is_some() {
            if self.random_seed.is_none() {
                self.random_seed = args.random_seed;
            } else if self.random_seed != args.random_seed {
                anyhow::bail!("Conflicing --random-seeed specified");
            }
        }
        if args.compose {
            anyhow::bail!("--compose must be the first argument");
        }
        if args.scenario {
            if args.spec2.is_some() {
                anyhow::bail!("In --scenario mode only one argument is expected")
            }
            if self.scenario.is_some() {
                anyhow::bail!("Conflicting --scenario options");
            }
            self.scenario = Some(args.spec1.clone().into());
        }
        if args.dump_spec {
            self.dump_spec = true
        }
        if args.dump_spec_phase0 {
            self.dump_spec_phase0 = true;
        }
        Ok(())
    }
}
