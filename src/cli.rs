use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
/// Tool to connect to WebSocket, listen them and do other network tricks
#[command(version, about)]
pub struct WebsocatArgs {
    pub spec1: String,

    pub spec2: Option<String>,

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
}
