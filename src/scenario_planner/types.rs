use std::net::SocketAddr;

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
    UdpConnect(SocketAddr),
    UdpBind(SocketAddr),
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
