#![allow(renamed_and_removed_lints)]
#![allow(unknown_lints)]
#![cfg_attr(feature = "cargo-clippy", allow(deprecated_cfg_attr))]

#[macro_use]
extern crate websocat;

extern crate futures;
extern crate tokio;
extern crate tokio_stdin_stdout;

extern crate websocket_base;

extern crate env_logger;
#[macro_use]
extern crate log;

#[cfg(feature = "openssl-probe")]
extern crate openssl_probe;

#[allow(unused_imports)]
#[macro_use]
extern crate structopt;

extern crate atty;

extern crate http_bytes;
use http_bytes::http;

use std::net::{IpAddr, SocketAddr};

use structopt::StructOpt;

use websocat::options::StaticFile;
use websocat::socks5_peer::{SocksHostAddr, SocksSocketAddr};
use websocat::{Options, SpecifierClass, WebsocatConfiguration1};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use std::ffi::OsString;

#[derive(StructOpt, Debug)]
#[structopt(
    after_help = "
Basic examples:
  Command-line websocket client:
    websocat ws://ws.vi-server.org/mirror/
    
  WebSocket server
    websocat -s 8080
    
  WebSocket-to-TCP proxy:
    websocat --binary ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    
",
    usage = "websocat ws://URL | wss://URL               (simple client)\n    websocat -s port                            (simple server)\n    websocat [FLAGS] [OPTIONS] <addr1> <addr2>  (advanced mode)"
)]
struct Opt {
    /// In simple mode, WebSocket URL to connect.
    /// In advanced mode first address (there are many kinds of addresses) to use.
    /// See --help=types for info about address types.
    /// If this is an address for listening, it will try serving multiple connections.
    addr1: Option<String>,
    /// In advanced mode, second address to connect.
    /// If this is an address for listening, it will accept only one connection.
    addr2: Option<String>,

    #[structopt(
        short = "u",
        long = "unidirectional",
        help = "Inhibit copying data in one direction"
    )]
    unidirectional: bool,
    #[structopt(
        short = "U",
        long = "unidirectional-reverse",
        help = "Inhibit copying data in the other direction (or maybe in both directions if combined with -u)"
    )]
    unidirectional_reverse: bool,

    #[structopt(
        long = "exit-on-eof",
        short = "E",
        help = "Close a data transfer direction if the other one reached EOF"
    )]
    exit_on_eof: bool,

    #[structopt(
        short = "t",
        long = "text",
        help = "Send message to WebSockets as text messages"
    )]
    websocket_text_mode: bool,

    #[structopt(
        short = "b",
        long = "binary",
        help = "Send message to WebSockets as binary messages"
    )]
    websocket_binary_mode: bool,

    #[structopt(
        long = "oneshot",
        help = "Serve only once. Not to be confused with -1 (--one-message)"
    )]
    oneshot: bool,

    #[structopt(
        short = "h",
        long = "help",
        help = "See the help.\n--help=short is the list of easy options and address types\n--help=long lists all options and types (see [A] markers)\n--help=doc also shows longer description and examples."
    )]
    help: Option<String>,

    #[structopt(
        long = "dump-spec",
        help = "[A] Instead of running, dump the specifiers representation to stdout"
    )]
    dumpspec: bool,

    /// Specify this Sec-WebSocket-Protocol: header when connecting
    #[structopt(long = "protocol")]
    websocket_protocol: Option<String>,

    /// Force this Sec-WebSocket-Protocol: header when accepting a connection
    #[structopt(long = "server-protocol")]
    websocket_reply_protocol: Option<String>,

    #[structopt(
        long = "udp-oneshot",
        help = "[A] udp-listen: replies only one packet per client"
    )]
    udp_oneshot_mode: bool,

    /// [A] Set SO_BROADCAST
    #[structopt(long="udp-broadcast")]
    udp_broadcast: bool,

    /// [A] Set IP[V6]_MULTICAST_LOOP
    #[structopt(long="udp-multicast-loop")]
    udp_multicast_loop: bool,

    /// [A] Set IP_TTL, also IP_MULTICAST_TTL if applicable
    #[structopt(long="udp-ttl")]
    udp_ttl: Option<u32>,

    /// [A] Issue IP[V6]_ADD_MEMBERSHIP for specified multicast address.
    /// Can be specified multiple times.
    #[structopt(long="udp-multicast")]
    udp_join_multicast_addr: Vec<std::net::IpAddr>,

    /// [A] IPv4 address of multicast network interface.
    /// Has to be either not specified or specified the same number of times as multicast IPv4 addresses. Order matters.
    #[structopt(long="udp-multicast-iface-v4")]
    udp_join_multicast_iface_v4: Vec<std::net::Ipv4Addr>,

    /// [A] Index of network interface for IPv6 multicast.
    /// Has to be either not specified or specified the same number of times as multicast IPv6 addresses. Order matters.
    #[structopt(long="udp-multicast-iface-v6")]
    udp_join_multicast_iface_v6: Vec<u32>,

    /// [A] Set SO_REUSEADDR for UDP socket. Listening TCP sockets are always reuseaddr.
    #[structopt(long="udp-reuseaddr")]
    udp_reuseaddr: bool,

    #[structopt(
        long = "unlink",
        help = "[A] Unlink listening UNIX socket before binding to it"
    )]
    unlink_unix_socket: bool,

    #[structopt(
        long = "accept-from-fd",
        help = "[A] Do not call `socket(2)` in UNIX socket listener peer, start with `accept(2)` using specified file descriptor number as argument instead of filename"
    )]
    unix_socket_accept_from_fd: bool,

    #[structopt(
        long = "exec-args",
        raw(allow_hyphen_values = r#"true"#),
        help = "[A] Arguments for the `exec:` specifier. Must be the last option, everything after it gets into the exec args list."
    )]
    exec_args: Vec<String>,

    #[structopt(
        long = "ws-c-uri",
        help = "[A] URI to use for ws-c: overlay",
        default_value = "ws://0.0.0.0/"
    )]
    ws_c_uri: String,

    #[structopt(
        long = "linemode-strip-newlines",
        help = "[A] Don't include trailing \\n or \\r\\n coming from streams in WebSocket messages"
    )]
    linemode_strip_newlines: bool,

    #[structopt(
        long = "--no-line",
        help = "[A] Don't automatically insert line-to-message transformation"
    )]
    no_auto_linemode: bool,

    #[structopt(
        long = "origin",
        help = "Add Origin HTTP header to websocket client request"
    )]
    origin: Option<String>,

    #[structopt(
        long = "header",
        short = "H",
        help = "Add custom HTTP header to websocket client request. Separate header name and value with a colon and optionally a single space. Can be used multiple times. Note that single -H may eat multiple further arguments, leading to confusing errors. Specify headers at the end or with equal sign like -H='X: y'.",
        parse(try_from_str = "interpret_custom_header")
    )]
    custom_headers: Vec<(String, Vec<u8>)>,

    #[structopt(
        long = "server-header",
        help = "Add custom HTTP header to websocket upgrade reply. Separate header name and value with a colon and optionally a single space. Can be used multiple times. Note that single -H may eat multiple further arguments, leading to confusing errors.",
        parse(try_from_str = "interpret_custom_header")
    )]
    custom_reply_headers: Vec<(String, Vec<u8>)>,

    /// Forward specified incoming request header to
    /// H_* environment variable for `exec:`-like specifiers.
    #[structopt(
        long = "header-to-env",
    )]
    headers_to_env: Vec<String>,

    #[structopt(
        long = "websocket-version",
        help = "Override the Sec-WebSocket-Version value"
    )]
    websocket_version: Option<String>,

    #[structopt(
        long = "no-close",
        short = "n",
        help = "Don't send Close message to websocket on EOF"
    )]
    websocket_dont_close: bool,

    #[structopt(
        short = "1",
        long = "one-message",
        help = "Send and/or receive only one message. Use with --no-close and/or -u/-U."
    )]
    one_message: bool,

    #[structopt(
        short = "s",
        long = "server-mode",
        help = "Simple server mode: specify TCP port or addr:port as single argument"
    )]
    server_mode: bool,

    #[structopt(
        long = "no-fixups",
        help = "[A] Don't perform automatic command-line fixups. May destabilize websocat operation. Use --dump-spec without --no-fixups to discover what is being inserted automatically and read the full manual about Websocat internal workings."
    )]
    no_lints: bool,

    #[structopt(
        short = "B",
        long = "buffer-size",
        help = "Maximum message size, in bytes",
        default_value = "65536"
    )]
    buffer_size: usize,

    #[structopt(
        short = "v",
        parse(from_occurrences),
        help = "Increase verbosity level to info or further"
    )]
    verbosity: u8,

    #[structopt(
        short = "q",
        help = "Suppress all diagnostic messages, except of startup errors"
    )]
    quiet: bool,

    #[structopt(
        long = "queue-len",
        help = "[A] Number of pending queued messages for broadcast reuser",
        default_value = "16"
    )]
    broadcast_queue_len: usize,

    #[structopt(
        short = "S",
        long = "strict",
        help = "strict line/message mode: drop too long messages instead of splitting them, drop incomplete lines."
    )]
    strict_mode: bool,

    #[structopt(
        short = "0",
        long = "null-terminated",
        help = "Use \\0 instead of \\n for linemode"
    )]
    linemode_zero_terminated: bool,

    #[structopt(
        long = "restrict-uri",
        help = "When serving a websocket, only accept the given URI, like `/ws`\nThis liberates other URIs for things like serving static files or proxying."
    )]
    restrict_uri: Option<String>,

    #[structopt(
        short = "F",
        long = "static-file",
        help = "Serve a named static file for non-websocket connections.\nArgument syntax: <URI>:<Content-Type>:<file-path>\nArgument example: /index.html:text/html:index.html\nDirectories are not and will not be supported for security reasons.\nCan be specified multiple times. Recommended to specify them at the end or with equal sign like `-F=...`, otherwise this option may eat positional arguments",
        parse(try_from_str = "interpret_static_file")
    )]
    serve_static_files: Vec<StaticFile>,

    #[structopt(
        short = "e",
        long = "set-environment",
        help = "Set WEBSOCAT_* environment variables when doing exec:/cmd:/sh-c:\nCurrently it's WEBSOCAT_URI and WEBSOCAT_CLIENT for\nrequest URI and client address (if TCP)\nBeware of ShellShock or similar security problems."
    )]
    exec_set_env: bool,

    #[structopt(
        long = "reuser-send-zero-msg-on-disconnect",
        help = "[A] Make reuse-raw: send a zero-length message to the peer when some clients disconnects."
    )]
    reuser_send_zero_msg_on_disconnect: bool,

    #[structopt(
        long = "exec-sighup-on-zero-msg",
        help = "[A] Make exec: or sh-c: or cmd: send SIGHUP on UNIX when facing incoming zero-length message."
    )]
    process_zero_sighup: bool,

    #[structopt(
        long = "exec-sighup-on-stdin-close",
        help = "[A] Make exec: or sh-c: or cmd: send SIGHUP on UNIX when input is closed."
    )]
    process_exit_sighup: bool,

    #[structopt(
        long = "exec-exit-on-disconnect",
        help = "[A] Make exec: or sh-c: or cmd: immediately exit when connection is closed, don't wait for termination."
    )]
    process_exit_on_disconnect: bool,

    #[structopt(
        long = "jsonrpc",
        help = "Format messages you type as JSON RPC 2.0 method calls. First word becomes method name, the rest becomes parameters, possibly automatically wrapped in []."
    )]
    jsonrpc: bool,

    #[structopt(
        long = "socks5-destination",
        help = "[A] Examples: 1.2.3.4:5678  2600:::80  hostname:5678",
        parse(try_from_str = "interpret_socks_destination")
    )]
    socks_destination: Option<SocksSocketAddr>,

    #[structopt(
        long = "socks5",
        help = "Use specified address:port as a SOCKS5 proxy. Note that proxy authentication is not supported yet. Example: --socks5 127.0.0.1:9050"
    )]
    auto_socks5: Option<SocketAddr>,

    #[structopt(
        long = "socks5-bind-script",
        help = "[A] Execute specified script in `socks5-bind:` mode when remote port number becomes known.",
        parse(from_os_str)
    )]
    socks5_bind_script: Option<OsString>,

    #[structopt(
        long = "tls-domain",
        alias = "ssl-domain",
        help = "[A] Specify domain for SNI or certificate verification when using tls-connect: overlay"
    )]
    tls_domain: Option<String>,

    #[cfg(feature = "ssl")]
    #[structopt(
        long = "pkcs12-der",
        help = "Pkcs12 archive needed to accept SSL connections, certificate and key.\nA command to output it: openssl pkcs12 -export -out output.pkcs12 -inkey key.pem -in cert.pem\nUse with -s (--server-mode) option or with manually specified TLS overlays.\nSee moreexamples.md for more info.",
        parse(try_from_os_str = "websocat::ssl_peer::interpret_pkcs12")
    )]
    pkcs12_der: Option<Vec<u8>>,

    #[cfg(feature = "ssl")]
    #[structopt(
        long = "pkcs12-passwd",
        help = "Password for --pkcs12-der pkcs12 archive. Required on Mac."
    )]
    pkcs12_passwd: Option<String>,

    #[cfg(feature = "ssl")]
    #[structopt(
        long = "client-pkcs12-der",
        help = "[A] Client identity TLS certificate",
        parse(try_from_os_str = "websocat::ssl_peer::interpret_pkcs12")
    )]
    client_pkcs12_der: Option<Vec<u8>>,

    #[cfg(feature = "ssl")]
    #[structopt(
        long = "client-pkcs12-passwd",
        help = "[A] Password for --client-pkcs12-der pkcs12 archive. Required on Mac."
    )]
    client_pkcs12_passwd: Option<String>,

    #[cfg(feature = "ssl")]
    #[structopt(
        long = "insecure",
        short = "k",
        help = "Accept invalid certificates and hostnames while connecting to TLS"
    )]
    tls_insecure: bool,

    /// Maximum number of simultaneous connections for listening mode
    #[structopt(long = "conncap")]
    max_parallel_conns: Option<usize>,

    /// Send WebSocket pings each this number of seconds
    #[structopt(long = "ping-interval")]
    ws_ping_interval: Option<u64>,

    /// Drop WebSocket connection if Pong message not received for this number of seconds
    #[structopt(long = "ping-timeout")]
    ws_ping_timeout: Option<u64>,
    
    /// [A] Just a Sec-WebSocket-Key value without running main Websocat
    #[structopt(long = "just-generate-key")]
    just_generate_key: bool,

    /// [A] Just a Sec-WebSocket-Accept value based on supplied
    /// Sec-WebSocket-Key value without running main Websocat
    #[structopt(long = "just-generate-accept")]
    just_generate_accept: Option<String>,

    /// [A] URI to use for `http-request:` specifier
    #[structopt(long = "request-uri")]
    request_uri: Option<http::Uri>,

    /// [A] Method to use for `http-request:` specifier
    #[structopt(long = "request-method", short="X")]
    request_method: Option<http::Method>,

    /// [A] Specify HTTP request headers for `http-request:` specifier.
    #[structopt(
        long = "request-header", 
        parse(try_from_str = "interpret_custom_header2"),
    )]
    request_headers: Vec<(http::header::HeaderName, http::header::HeaderValue)>,

    /// [A] Don't exit when encountered a zero message.
    /// Zero messages are used internally in Websocat,
    /// so it may fail to close connection at all.
    #[structopt(long = "no-exit-on-zeromsg")]
    no_exit_on_zeromsg: bool,

    /// [A] Silently drop incoming zero-length WebSocket messages.
    /// They may cause connection close due to
    /// usage of zero-len message as EOF flag inside Websocat.
    #[structopt(long = "websocket-ignore-zeromsg")]
    websocket_ignore_zeromsg: bool,
    
    /// Maximum number of messages to copy in one direction.
    #[structopt(long = "max-messages")]
    max_messages: Option<usize>,

    /// Maximum number of messages to copy in the other direction.
    #[structopt(long = "max-messages-rev")]
    max_messages_rev: Option<usize>,

    /// [A] Delay before reconnect attempt for `autoreconnect:` overlay.
    #[structopt(long = "--autoreconnect-delay-millis", default_value="20")]
    autoreconnect_delay_millis: u64,


    /// [A] Prepend specified text to each received WebSocket text message.
    /// Also strip this prefix from outgoing messages, explicitly marking
    /// them as text even if `--binary` is specified
    #[structopt(long = "--text-prefix")]
    pub ws_text_prefix: Option<String>,
    
    /// [A] Prepend specified text to each received WebSocket binary message.
    /// Also strip this prefix from outgoing messages, explicitly marking
    /// them as binary even if `--text` is specified
    #[structopt(long = "--binary-prefix")]
    pub ws_binary_prefix: Option<String>,

    /// Encode incoming binary WebSocket messages in one-line Base64
    /// If `--binary-prefix` (see `--help=full`) is set, outgoing WebSocket messages
    /// that start with the prefix are decoded from base64 prior to sending.
    #[structopt(long = "--base64")]
    pub ws_binary_base64: bool,

    /// [A] Encode incoming text WebSocket messages in one-line Base64.
    /// I don't know whether it can be ever useful, but it's for symmetry with `--base64`.
    #[structopt(long = "--base64-text")]
    pub ws_text_base64: bool,

    /// Close connection with a status code.
    #[structopt(long = "--close-status-code")]
    pub close_status_code: Option<u16>,

    /// Close connection with a reason message. This option only takes effect if
    /// --close-status-code option is provided as well.
    #[structopt(long = "--close-reason")]
    pub close_reason: Option<String>,

    /// [A] On UNIX, set stdin and stdout to nonblocking mode instead of spawning a thread.
    /// This should improve performance, but may break other programs running on the same console.
    #[structopt(long = "--async-stdio")]
    pub asyncstdio: bool,

    /// [A] Inhibit using stdin/stdout in a nonblocking way if it is not a tty 
    #[structopt(long = "--no-async-stdio")]
    pub noasyncstdio: bool,

    /// Add `Authorization: Basic` HTTP request header with this base64-encoded parameter
    #[structopt(long = "--basic-auth")]
    pub basic_auth: Option<String>,

    /// [A] Wait for reading to finish before closing foreachmsg:'s peer
    #[structopt(long = "--foreachmsg-wait-read")]
    pub foreachmsg_wait_reads: bool,

    /// [A] Print a line to stdout for each port being listened
    #[structopt(long = "--stdout-announce-listening-ports")]
    pub announce_listens: bool,

    /// [A] Use monotonic clock for `timestamp:` overlay
    #[structopt(long = "--timestamp-monotonic")]
     pub timestamp_monotonic: bool,

    /// Print measured round-trip-time to stderr after each received WebSocket pong.
    #[structopt(long = "print-ping-rtts")]
    pub print_ping_rtts: bool,

    /// [A] Specify encryption/decryption key for `crypto:` specifier. Requires `base64:`, `file:` or `pwd:` prefix.
    #[cfg(feature = "crypto_peer")]
    #[structopt(long = "crypto-key", parse(try_from_str = "websocat::crypto_peer::interpret_opt"))]
    pub crypto_key: Option<[u8; 32]>,

    /// [A] Swap encryption and decryption operations in `crypto:` specifier - encrypt on read, decrypto on write.
    #[cfg(feature = "crypto_peer")]
    #[structopt(long = "crypto-reverse")]
    pub crypto_reverse: bool,

    /// Expose Prometheus metrics on specified IP address and port in addition to running usual Websocat session
    #[cfg(feature = "prometheus_peer")]
    #[structopt(long = "prometheus")]
    pub prometheus: Option<SocketAddr>,

    /// [A] Override the byte which byte_to_exit_on: overlay looks for
    #[structopt(long = "byte-to-exit-on", default_value = "28")]
    byte_to_exit_on: u8,

    /// [A] Maximum size of incoming WebSocket messages (sans of one data frame), to prevent memory overflow
    #[structopt(long = "max-ws-message-length", default_value = "209715200")]
    pub max_ws_message_length: usize,
    /// [A] Maximum size of incoming WebSocket frames, to prevent memory overflow
    #[structopt(long = "max-ws-frame-length", default_value = "104857600")]
    pub max_ws_frame_length: usize,

    /// Prepend copied data with a specified string. Can be specified multiple times.
    #[structopt(long = "preamble", short="p")]
    pub preamble: Vec<String>,

    /// Prepend copied data with a specified string (reverse direction). Can be specified multiple times.
    #[structopt(long = "preamble-reverse", short="P")]
    pub preamble_reverse: Vec<String>,


    /// [A] Compress data coming to a WebSocket using deflate method. Affects only binary WebSocket messages.
    #[structopt(long = "compress-deflate")]
    pub compress_deflate: bool,

    /// [A] Compress data coming to a WebSocket using zlib method. Affects only binary WebSocket messages.
    #[structopt(long = "compress-zlib")]
    pub compress_zlib: bool,

    /// [A] Compress data coming to a WebSocket using gzip method. Affects only binary WebSocket messages.
    #[structopt(long = "compress-gzip")]
    pub compress_gzip: bool,

    /// [A] Uncompress data coming from a WebSocket using deflate method. Affects only binary WebSocket messages.
    #[structopt(long = "uncompress-deflate")]
    pub uncompress_deflate: bool,

    /// [A] Uncompress data coming from a WebSocket using deflate method. Affects only binary WebSocket messages.
    #[structopt(long = "uncompress-zlib")]
    pub uncompress_zlib: bool,

    /// [A] Uncompress data coming from a WebSocket using deflate method. Affects only binary WebSocket messages.
    #[structopt(long = "uncompress-gzip")]
    pub uncompress_gzip: bool,

    /// [A] Load specified symbol from specified native library and use it for `native_plugin_transform_a`.
    /// Format is `symbol@library_file`. If `symbol@` is omitted, `websocat_transform` is implied.
    #[cfg(feature = "native_plugins")]
    #[structopt(long = "native-plugin-a",  parse(try_from_str = "websocat::transform_peer::load_symbol"))]
    pub native_transform_a: Option<websocat::transform_peer::Sym>,

    /// [A] Load specified symbol from specified native library and use it for `native_plugin_transform_b`.
    #[cfg(feature = "native_plugins")]
    #[structopt(long = "native-plugin-b", parse(try_from_str = "websocat::transform_peer::load_symbol"))]
    pub native_transform_b: Option<websocat::transform_peer::Sym>,

    /// [A] Load specified symbol from specified native library and use it for `native_plugin_transform_c`.
    #[cfg(feature = "native_plugins")]
    #[structopt(long = "native-plugin-c", parse(try_from_str = "websocat::transform_peer::load_symbol"))]
    pub native_transform_c: Option<websocat::transform_peer::Sym>,

    /// [A] Load specified symbol from specified native library and use it for `native_plugin_transform_d`.
    #[cfg(feature = "native_plugins")]
    #[structopt(long = "native-plugin-d", parse(try_from_str = "websocat::transform_peer::load_symbol"))]
    pub native_transform_d: Option<websocat::transform_peer::Sym>,

    /// [A] Load specified symbol from specified wasm module and use it for `wasm_plugin_transform_a:`.
    /// Format is `symbol@library_file`. If `symbol@` is omitted, `websocat_transform` is implied.
    /// The wasm module should also have `malloc` and `free` functions exposed.
    /// Prepend `library_file` with `!` to load serialized cwasm produced by `wasmtime compile` instead of compining the module in Websocat.
    #[cfg(feature = "wasm_plugins")]
    #[structopt(long = "wasm-plugin-a",  parse(try_from_str = "websocat::wasm_transform_peer::load_symbol"))]
    pub wasm_transform_a: Option<websocat::wasm_transform_peer::Handle>,

    /// [A] Load specified symbol from specified wasm module and use it for `wasm_plugin_transform_b:`.
    #[cfg(feature = "wasm_plugins")]
    #[structopt(long = "wasm-plugin-b",  parse(try_from_str = "websocat::wasm_transform_peer::load_symbol"))]
    pub wasm_transform_b: Option<websocat::wasm_transform_peer::Handle>,

    /// [A] Load specified symbol from specified wasm module and use it for `wasm_plugin_transform_c:`.
    #[cfg(feature = "wasm_plugins")]
    #[structopt(long = "wasm-plugin-c",  parse(try_from_str = "websocat::wasm_transform_peer::load_symbol"))]
    pub wasm_transform_c: Option<websocat::wasm_transform_peer::Handle>,

    /// [A] Load specified symbol from specified wasm module and use it for `wasm_plugin_transform_d:`.
    #[cfg(feature = "wasm_plugins")]
    #[structopt(long = "wasm-plugin-d",  parse(try_from_str = "websocat::wasm_transform_peer::load_symbol"))]
    pub wasm_transform_d: Option<websocat::wasm_transform_peer::Handle>,

    /// [A] Omit `jsonrpc` field when using `--jsonrpc`, e.g. for Chromium
    #[structopt(long = "jsonrpc-omit-jsonrpc")]
    pub jsonrpc_omit_jsonrpc: bool,

    /// [A] Stop replying to incoming WebSocket pings after specified number of replies 
    #[structopt(long = "inhibit-pongs")]
    pub inhibit_pongs: Option<usize>,

    /// [A] Stop sending pings after this number of sent pings
    #[structopt(long = "max-sent-pings")]
    pub max_sent_pings: Option<usize>,

    /// [A] Use this number of length header bytes for `lengthprefixed:` overlay.
    #[structopt(long = "--lengthprefixed-nbytes", default_value = "4")]
    pub lengthprefixed_header_bytes: usize,

    /// [A] Use little-endian framing headers instead of big-endian for `lengthprefixed:` overlay.
    #[structopt(long = "--lengthprefixed-little-endian")]
    pub lengthprefixed_little_endian: bool,
}

// TODO: make it byte-oriented/OsStr?
fn interpret_custom_header(x: &str) -> Result<(String, Vec<u8>)> {
    let colon = x.find(':');
    let colon = if let Some(colon) = colon {
        colon
    } else {
        Err("Argument to --header must contain `:` character")?
    };
    let hn = &x[0..colon];
    let mut hv = &x[colon + 1..];
    if hv.starts_with(' ') {
        hv = &x[colon + 2..];
    }
    Ok((hn.to_owned(), hv.as_bytes().to_vec()))
}

fn interpret_custom_header2(x: &str) -> Result<(http::header::HeaderName, http::header::HeaderValue)> {
    let colon = x.find(':');
    let colon = if let Some(colon) = colon {
        colon
    } else {
        Err("Specified header must contain `:` character")?
    };
    let hn = &x[0..colon];
    let mut hv = &x[colon + 1..];
    if hv.starts_with(' ') {
        hv = &x[colon + 2..];
    }
    use std::str::FromStr;
    let hn = http::header::HeaderName::from_str(hn)?;
    let hv = http::header::HeaderValue::from_str(hv)?;
    Ok((hn,hv))
}

fn interpret_static_file(x: &str) -> Result<StaticFile> {
    let colon1 = match x.find(':') {
        Some(x) => x,
        None => Err("Argument to --static-file must contain two colons (`:`)")?,
    };
    let uri = &x[0..colon1];
    let rest = &x[colon1 + 1..];
    let colon2 = match rest.find(':') {
        Some(x) => x,
        None => Err("Argument to --static-file must contain two colons (`:`)")?,
    };
    let ct = &rest[0..colon2];
    let fp = &rest[colon2 + 1..];
    if uri.is_empty() || ct.is_empty() || fp.is_empty() {
        Err("Empty URI, content-type or path in --static-file parameter")?
    }
    Ok(StaticFile {
        uri: uri.to_string(),
        content_type: ct.to_string(),
        file: fp.into(),
    })
}

fn interpret_socks_destination(x: &str) -> Result<SocksSocketAddr> {
    let colon = x.rfind(':');
    let colon = if let Some(colon) = colon {
        colon
    } else {
        Err("Argument to --socks5-destination must contain a `:` character")?
    };
    let h = &x[0..colon];
    let p = &x[colon + 1..];

    let port: u16 = p.parse()?;

    let host = if let Ok(ip4) = h.parse() {
        SocksHostAddr::Ip(IpAddr::V4(ip4))
    } else if let Ok(ip6) = h.parse() {
        SocksHostAddr::Ip(IpAddr::V6(ip6))
    } else {
        SocksHostAddr::Name(h.to_string())
    };

    Ok(SocksSocketAddr { host, port })
}

pub mod help;

// Based on https://github.com/rust-clique/clap-verbosity-flag/blob/master/src/lib.rs
mod logging {

    extern crate env_logger;
    extern crate log;

    use self::env_logger::Builder as LoggerBuilder;
    use self::log::Level;

    pub fn setup_env_logger(ll: u8) -> Result<(), Box<dyn (::std::error::Error)>> {
        if ::std::env::var("RUST_LOG").is_ok() {
            if ll > 0 {
                eprintln!("websocat: RUST_LOG environment variable overrides any -v");
            }
            env_logger::init();
            return Ok(());
        }

        let lf = match ll {
            //0 => Level::Error,
            0 => Level::Warn,
            1 => Level::Info,
            2 => Level::Debug,
            _ => Level::Trace,
        }
        .to_level_filter();

        LoggerBuilder::new()
            .filter(Some("websocat"), lf)
            .filter(None, Level::Warn.to_level_filter())
            .try_init()?;
        Ok(())
    }

}

fn run() -> Result<()> {
    if std::env::args().nth(1).unwrap_or_default() == "--long-help" {
        help::longhelp();
        return Ok(());
    }
    if vec!["-?", "-h", "--help"].contains(&std::env::args().nth(1).unwrap_or_default().as_str()) {
        help::shorthelp();
        return Ok(());
    }
    let mut logging_already_set = false;
    if std::env::var("WEBSOCAT_EARLY_LOG").is_ok() {
        logging::setup_env_logger(0)?;
        logging_already_set = true;
    }

    let mut cmd = Opt::from_args();

    let mut quiet = cmd.quiet;

    if let Some(h) = cmd.help {
        if &h == "long" || &h == "full" || &h == "all" {
            help::longhelp();
            return Ok(());
        } else if &h == "doc" {
            help::dochelp();
            return Ok(());
        }

        help::shorthelp();
        return Ok(());
    }

    if cmd.just_generate_key {
        println!("{}", websocket_base::header::WebSocketKey::new().serialize());
        return Ok(());
    }

    if let Some(key) = cmd.just_generate_accept {
        use std::str::FromStr;
        let k = websocket_base::header::WebSocketKey::from_str(&key)?;
        println!("{}", websocket_base::header::WebSocketAccept::new(&k).serialize());
        return Ok(());
    }

    let mut recommend_explicit_text_or_bin = false;

    if cmd.websocket_binary_mode && cmd.websocket_text_mode {
        Err("--binary and --text are mutually exclusive")?;
    }
    if !cmd.websocket_binary_mode && !cmd.websocket_text_mode {
        cmd.websocket_text_mode = true;
        recommend_explicit_text_or_bin = true;
    }

    if cmd.noasyncstdio && cmd.asyncstdio {
        Err("--no-async-stdio and --async-stdio are not meaningful together")?;
    }

    if ! cmd.noasyncstdio {
        if atty::isnt(atty::Stream::Stdin) && atty::isnt(atty::Stream::Stdout) {
            cmd.asyncstdio = true;
        }
    }

    //if !cmd.serve_static_files.is_empty() && cmd.restrict_uri.is_none() {
    //    Err("Specify --static-file is not supported without --restrict-uri")?
    //}

    if false
    //    || cmd.oneshot
    {
        Err("This mode is not implemented")?
    }

    #[cfg(feature = "openssl-probe")]
    {
        openssl_probe::init_ssl_cert_env_vars();
    }

    let mut opts: Options = Default::default();
    {
        macro_rules! opts {
            ($($o:ident)*) => {{
                $(opts.$o = cmd.$o;)*
            }};
        }
        opts!(
            websocket_text_mode
            websocket_protocol
            websocket_reply_protocol
            udp_oneshot_mode
            udp_broadcast
            udp_multicast_loop
            udp_ttl
            udp_join_multicast_addr
            udp_join_multicast_iface_v4
            udp_join_multicast_iface_v6
            udp_reuseaddr
            unidirectional
            unidirectional_reverse
            exit_on_eof
            oneshot
            unlink_unix_socket
            unix_socket_accept_from_fd
            exec_args
            ws_c_uri
            linemode_strip_newlines
            origin
            custom_headers
            custom_reply_headers
            headers_to_env
            websocket_version
            websocket_dont_close
            one_message
            no_auto_linemode
            buffer_size
            linemode_zero_terminated
            broadcast_queue_len
            restrict_uri
            serve_static_files
            exec_set_env
            reuser_send_zero_msg_on_disconnect
            process_zero_sighup
            process_exit_sighup
            process_exit_on_disconnect
            socks_destination
            auto_socks5
            socks5_bind_script
            tls_domain
            max_parallel_conns
            ws_ping_interval
            ws_ping_timeout
            request_uri
            request_method
            request_headers
            websocket_ignore_zeromsg
            no_exit_on_zeromsg
            max_messages
            max_messages_rev
            autoreconnect_delay_millis
            ws_text_prefix
            ws_binary_prefix
            ws_binary_base64
            ws_text_base64
            close_status_code
            close_reason
            asyncstdio
            foreachmsg_wait_reads
            announce_listens
            timestamp_monotonic
            print_ping_rtts
            byte_to_exit_on
            max_ws_message_length
            max_ws_frame_length
            preamble
            preamble_reverse
            compress_deflate
            compress_zlib
            compress_gzip
            uncompress_deflate
            uncompress_zlib
            uncompress_gzip
            jsonrpc_omit_jsonrpc
            inhibit_pongs
            max_sent_pings
            lengthprefixed_header_bytes
            lengthprefixed_little_endian
        );
        #[cfg(feature = "ssl")]
        {
            opts! {
                pkcs12_der
                pkcs12_passwd
                client_pkcs12_der
                client_pkcs12_passwd
                tls_insecure
            }
        }
        #[cfg(feature = "crypto_peer")]
        {
            opts! {
                crypto_key
                crypto_reverse
            }
        }
        #[cfg(feature = "prometheus_peer")]
        {
            opts! {
                prometheus
            }
        }
        #[cfg(feature = "native_plugins")]
        {
            opts! {
                native_transform_a
                native_transform_b
                native_transform_c
                native_transform_d
            }
        }
        #[cfg(feature = "wasm_plugins")]
        {
            opts! {
                 wasm_transform_a
                 wasm_transform_b
                 wasm_transform_c
                 wasm_transform_d
            }
        }
    };

    if let Some(ba) = cmd.basic_auth {
        let x = base64::encode(&ba);
        let q = format!("Basic {}", x);
        opts.custom_headers.push(("Authorization".to_owned(), q.as_bytes().to_vec()));
        opts.request_headers.push((http::header::AUTHORIZATION, http::header::HeaderValue::from_bytes(q.as_bytes()).unwrap()));
    }

    let (s1, s2): (String, String) = match (cmd.addr1, cmd.addr2) {
        (None, None) => {
            for x in std::env::args() {
                if x == "-p" || x == "-P" || x == "--preamble" || x == "--preamble-reverse" {
                    eprintln!("Warning: all dashless arguments after -p or -P are considered part of the preamble. You may want to move -p/-P to the end of the command line.")
                }
            }
            return Err("No URL specified. Use `websocat --help` to show the help message.")?;
        }
        (Some(cmds1), Some(cmds2)) => {
            // Advanced mode
            if cmd.jsonrpc {
                Err("--jsonrpc option is only for simple (single-argument) mode.\nUse `jsonrpc:` specifier manually if you want it in advanced mode.")?
            }
            if cmd.server_mode {
                Err("--server and two positional arguments are incompatible.\nBuild server command line without -s option, but with `listen` address types")?
            }
            (cmds1, cmds2)
        }
        (Some(cmds1), None) => {
            // Easy mode
            recommend_explicit_text_or_bin = false;
            if cmd.server_mode {
                #[allow(unused)]
                let mut secure = false;
                #[cfg(feature = "ssl")]
                {
                    if opts.pkcs12_der.is_some() {
                        secure = true;
                    }
                }

                opts.exit_on_eof = true;
                if !secure {
                    if cmds1.contains(':') {
                        if !quiet {
                            eprintln!("Listening on ws://{}/", cmds1);
                        }
                        (format!("ws-l:{}", cmds1), "-".to_string())
                    } else {
                        if !quiet {
                            eprintln!("Listening on ws://127.0.0.1:{}/", cmds1);
                        }
                        (format!("ws-l:127.0.0.1:{}", cmds1), "-".to_string())
                    }
                } else if cmds1.contains(':') {
                    if !quiet {
                        eprintln!("Listening on wss://{}/", cmds1);
                    }
                    (format!("wss-l:{}", cmds1), "-".to_string())
                } else {
                    if !quiet {
                        eprintln!("Listening on wss://127.0.0.1:{}/", cmds1);
                    }
                    (format!("wss-l:127.0.0.1:{}", cmds1), "-".to_string())
                }
            } else {
                if !(cmds1.starts_with("ws://") || cmds1.starts_with("wss://")) {
                    if !quiet {
                        eprintln!("Specify ws:// or wss:// URI to connect to a websocket");
                    }
                    Err("Invalid command-line parameters")?;
                }
                ("-".to_string(), cmds1)
            }
        }
        (None, Some(_)) => unreachable!(),
    };

    if opts.websocket_text_mode {
        opts.read_debt_handling = websocat::readdebt::DebtHandling::Warn;
    }
    if cmd.strict_mode {
        opts.read_debt_handling = websocat::readdebt::DebtHandling::DropMessage;
        opts.linemode_strict = true;
    }

    debug!("Done first phase of interpreting options.");
    let websocat1 = WebsocatConfiguration1 {
        opts,
        addr1: s1,
        addr2: s2,
    };
    let mut websocat2 = websocat1.parse1()?;
    debug!("Done second phase of interpreting options.");

    if websocat2.inetd_mode() {
        quiet = true;
    }

    if !quiet && recommend_explicit_text_or_bin {
        eprintln!("websocat: It is recommended to either set --binary or --text explicitly");
    }
    if !quiet && !logging_already_set {
        logging::setup_env_logger(cmd.verbosity)?;
    }

    if !cmd.no_lints {
        websocat2.lint_and_fixup(Box::new(move |e: &str| {
            if !quiet {
                eprintln!("websocat: {}", e);
            }
        }))?;
    }
    if cmd.jsonrpc {
        websocat2
            .s1
            .overlays
            .insert(0, websocat::specifier::SpecifierNode{cls: ::std::rc::Rc::new(websocat::jsonrpc_peer::JsonRpcClass)});
    }
    debug!("Done third phase of interpreting options.");
    let websocat = websocat2.parse2()?;
    debug!("Done fourth phase of interpreting options.");

    if cmd.dumpspec {
        println!("{:?}", websocat.s1);
        println!("{:?}", websocat.s2);
        println!("{:?}", websocat.opts);
        return Ok(());
    }

    let mut core = tokio::runtime::current_thread::Runtime::new()?;

    let error_handler = std::rc::Rc::new(move |e| {
        if !quiet {
            eprintln!("websocat: {}", e);
        }
    });
    let prog = websocat.serve(error_handler);
    debug!("Preparation done. Now actually starting.");
    core.block_on(prog)
        .map_err(|()| "error running".to_string())?;
    Ok(())
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
