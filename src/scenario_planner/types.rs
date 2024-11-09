use std::{ffi::OsString, net::SocketAddr};

use http::Uri;

use crate::cli::WebsocatArgs;

#[derive(Debug)]
pub enum Endpoint {
    //@ @inhibit_prefixes
    TcpConnectByEarlyHostname {
        varname_for_addrs: String,
    },
    //@ @inhibit_prefixes
    /// All TCP connections start as late-resolved when parsing CLI argument,
    /// but may be converted to early-resolved by the patcher.
    TcpConnectByLateHostname {
        hostname: String,
    },
    TcpConnectByIp(SocketAddr),
    TcpListen(SocketAddr),
    WsUrl(Uri),
    WssUrl(Uri),
    WsListen(SocketAddr),
    //@ Console, terminal: read bytes from stdin, write bytes to stdout.
    Stdio,
    //@ Connect to this UDP socket. Note affected by `--udp-bind-*`` CLI options.
    UdpConnect(SocketAddr),
    //@ Bind UDP socket to this address.
    //@ Commmand line options greatly affect this endpoint. It can be turned into a flexible UdpConnect analogue.
    UdpBind(SocketAddr),
    //@ Bind UDP socket and spawn a separate task for each client
    UdpServer(SocketAddr),
    //@ Execute given program as subprocess and use its stdin/stdout as a socket.
    //@ Specify command line arguments using `--exec-args` command line option.
    Exec(OsString),
    //@ Execute given command line and use its stdin/stdout as a socket.
    Cmd(OsString),
    //@ Byte stream socket that ignores all incoming data and immediately EOF-s read attempts
    DummyStream,
    //@ Datagram socket that ignores all incoming data and signals EOF immediately
    DummyDatagrams,
    //@ Byte stream socket that produces specified content and ignores incoming data
    Literal(String),
    //@ Byte stream socket that produces specified content (base64-encoded) and ignores incoming data
    LiteralBase64(String),

    //@ Connect to the specified UNIX socket path
    UnixConnect(OsString),
    //@ Listen specified UNIX socket path
    UnixListen(OsString),

    //@ Connect to the specified abstract-namespaced UNIX socket (Linux)
    AbstractConnect(OsString),
    //@ Listen UNIX socket on specified abstract path (Linux)
    AbstractListen(OsString),
}

#[derive(Debug)]
pub enum Overlay {
    WsUpgrade {
        uri: Uri,
        host: Option<String>,
    },
    WsAccept {},
    WsFramer {
        client_mode: bool,
    },
    //@ Combined WebSocket upgrader and framer, but without TCP or TLS things
    //@ URI is taked from --ws-c-uri CLI argument
    //@ If it is not specified, it defaults to `/`, with a missing `host:` header
    WsClient,
    //@ Combined WebSocket acceptor and framer.
    WsServer,
    TlsClient {
        domain: String,
        varname_for_connector: String,
    },
    StreamChunks,
    LineChunks,
    //@ Print encountered data to stderr for debugging
    Log {
        datagram_mode: bool,
    },
    //@ Limit this stream's read buffer size to --read-buffer-limit
    //@ By splitting reads to many (e.g. single byte) chunks, we can
    //@ test and debug trickier code paths in various overlays
    ReadChunkLimiter,
    //@ Limit this stream's write buffer size to --write-buffer-limit
    //@ By enforcing short writes, we can
    //@ test and debug trickier code paths in various overlays
    WriteChunkLimiter,
    //@ Insert write buffering layer that combines multiple write calls to one bigger
    WriteBuffer,
}

#[derive(Debug)]
pub struct SpecifierStack {
    pub innermost: Endpoint,
    /// zeroeth element is the last specified overlay, e.g. `ws-ll:` in `reuse:autoreconnect:ws-ll:tcp:127.0.0.1:1234`.
    pub overlays: Vec<Overlay>,
}

#[derive(Debug)]
pub enum PreparatoryAction {
    ResolveHostname {
        hostname: String,
        varname_for_addrs: String,
    },
    CreateTlsConnector {
        varname_for_connector: String,
    },
}

pub struct WebsocatInvocation {
    pub left: SpecifierStack,
    pub right: SpecifierStack,
    pub opts: WebsocatArgs,

    pub beginning: Vec<PreparatoryAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyingType {
    ByteStream,
    Datarams,
}
