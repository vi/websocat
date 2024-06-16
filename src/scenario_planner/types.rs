use std::net::SocketAddr;

use http::Uri;

use crate::cli::WebsocatArgs;

#[derive(Debug)]
pub enum Endpoint {
    TcpConnectByIp(SocketAddr),
    TcpListen(SocketAddr),
    WsUrl(Uri),
    WssUrl(Uri),
    Stdio,
    UdpConnect(SocketAddr),
    UdpBind(SocketAddr),
}


#[derive(Debug)]
pub enum Overlay {
    WsUpgrade(Uri),
    WsFramer{client_mode: bool},
    StreamChunks,
}


#[derive(Debug)]
pub struct SpecifierStack {
    pub innermost: Endpoint,
    /// zeroeth element is the last specified overlay, e.g. `ws-ll:` in `reuse:autoreconnect:ws-ll:tcp:127.0.0.1:1234`.
    pub overlays: Vec<Overlay>,
}


pub struct WebsocatInvocation {
    pub left: SpecifierStack,
    pub right: SpecifierStack,
    pub opts: WebsocatArgs,
}

#[derive(Debug,Clone, Copy,PartialEq, Eq)]
pub enum CopyingType {
    ByteStream,
    Datarams,
}
