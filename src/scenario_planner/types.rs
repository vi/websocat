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
        host: String,
    },
    WsAccept {},
    WsFramer {
        client_mode: bool,
    },
    TlsClient {
        domain: String,
        varname_for_connector: String,
    },
    StreamChunks,
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
