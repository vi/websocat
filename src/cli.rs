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
    #[arg(long)]
    pub dump_spec_phase1: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    #[arg(long)]
    pub dump_spec_phase2: bool,

    /// execute specified file as Rhai script (e.g. resutling from --dump-spec option output)
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

    /// listen for WebSocket conenctions instead of establishing client WebSocket connection
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

    /// automatically insert `log:` overlay in an apprioriate place to debug issues by displaying traffic chunks
    #[arg(long)]
    pub log_traffic: bool,

    /// URI for `ws-c:` overlay.
    #[arg(long)]
    pub ws_c_uri: Option<String>,

    /// paramemter for read_chunk_limiter: overlay, defaults to 1
    #[arg(long)]
    pub read_buffer_limit: Option<usize>,

    /// paramemter for write_chunk_limiter: overlay, defaults to 1
    #[arg(long)]
    pub write_buffer_limit: Option<usize>,

    /// override byte value that separates stdin-supplied text WebSocket messages
    /// from each othe from default '\n'.
    #[arg(long)]
    pub separator: Option<u8>,

    /// require this number of newline (or other) bytes to separate WebSocket messages
    #[arg(long)]
    pub separator_n: Option<usize>,

    /// prevent mangling incoming text WebSocket by replacing `\n`  (or other
    /// separator sequence) with spaces (and trimming leading and trailing separator bytes)
    #[arg(long)]
    pub separator_inhibit_substitution: bool,

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
    /// Note that some overlays and endpoints may have separate buffers with sepaparately adjustable sizes.
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

    /// Do not fail WebSocket connections if maksed frame arrives instead of unmasked or vice versa.
    #[arg(long)]
    pub ws_ignore_invalid_masks: bool,

    /// Ignore absense or invalid values of `Sec-Websocket-*` things and just continue connecting.
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

    /// Maximum size of an outgoing UDP datagram. Incoming datagram size is likely limited by --buffer-size.
    #[arg(long, default_value="4096")]
    pub udp_max_send_datagram_size: usize,

    /// Maximum size of an outgoing SEQPACKET datagram. Incoming datagram size is likely limited by --buffer-size.
    #[arg(long, default_value="1048576")]
    pub seqpacket_max_send_datagram_size: usize,
}
