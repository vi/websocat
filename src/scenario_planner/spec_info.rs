use tracing::debug;

use crate::cli::WebsocatArgs;

use super::types::{Endpoint, Overlay, SocketType, SpecifierStack, WebsocatInvocation};

impl WebsocatInvocation {
    pub fn session_socket_type(&self) -> SocketType {
        match (
            self.stacks.left.provides_socket_type(),
            self.stacks.right.provides_socket_type(),
        ) {
            (SocketType::ByteStream, SocketType::ByteStream) => SocketType::ByteStream,
            (SocketType::Datarams, SocketType::Datarams) => SocketType::Datarams,
            _ => {
                debug!("Incompatible types encountered: bytestream-oriented and datagram-oriented, falling back to datagrams");
                SocketType::Datarams
            }
        }
    }
}

impl SpecifierStack {
    pub(super) fn provides_socket_type(&self) -> SocketType {
        let mut typ = self.innermost.provides_socket_type();
        for ovl in &self.overlays {
            if let Some(t) = ovl.provides_socket_type() {
                typ = t
            }
        }
        typ
    }
}

impl Endpoint {
    pub(super) fn provides_socket_type(&self) -> SocketType {
        use SocketType::{ByteStream, Datarams};
        match self {
            Endpoint::TcpConnectByIp(_) => ByteStream,
            Endpoint::TcpListen(_) => ByteStream,
            Endpoint::TcpListenFd(_) => ByteStream,
            Endpoint::TcpListenFdNamed(_) => ByteStream,
            Endpoint::TcpConnectByEarlyHostname { .. } => ByteStream,
            Endpoint::TcpConnectByLateHostname { hostname: _ } => ByteStream,
            Endpoint::WsUrl(_) => Datarams,
            Endpoint::WssUrl(_) => Datarams,
            Endpoint::Stdio => ByteStream,
            Endpoint::UdpConnect(_) => Datarams,
            Endpoint::UdpBind(_) => Datarams,
            Endpoint::UdpFd(_) => Datarams,
            Endpoint::UdpFdNamed(_) => Datarams,
            Endpoint::WsListen(_) => Datarams,
            Endpoint::UdpServer(_) => Datarams,
            Endpoint::UdpServerFd(_) => Datarams,
            Endpoint::UdpServerFdNamed(_) => Datarams,
            Endpoint::Exec(_) => ByteStream,
            Endpoint::Cmd(_) => ByteStream,
            Endpoint::DummyStream => ByteStream,
            Endpoint::DummyDatagrams => Datarams,
            Endpoint::Literal(_) => ByteStream,
            Endpoint::LiteralBase64(_) => ByteStream,
            Endpoint::UnixConnect(_) => ByteStream,
            Endpoint::UnixListen(_) => ByteStream,
            Endpoint::AbstractConnect(_) => ByteStream,
            Endpoint::AbstractListen(_) => ByteStream,
            Endpoint::UnixListenFd(_) => ByteStream,
            Endpoint::UnixListenFdNamed(_) => ByteStream,
            Endpoint::SeqpacketConnect(_) => Datarams,
            Endpoint::SeqpacketListen(_) => Datarams,
            Endpoint::AbstractSeqpacketConnect(_) => Datarams,
            Endpoint::AbstractSeqpacketListen(_) => Datarams,
            Endpoint::SeqpacketListenFd(_) => Datarams,
            Endpoint::SeqpacketListenFdNamed(_) => Datarams,
            Endpoint::MockStreamSocket(_) => ByteStream,
            Endpoint::RegistryStreamListen(_) => ByteStream,
            Endpoint::RegistryStreamConnect(_) => ByteStream,
            Endpoint::AsyncFd(_) => ByteStream,
            Endpoint::SimpleReuserEndpoint(..) => Datarams,
            Endpoint::ReadFile(..) => ByteStream,
            Endpoint::WriteFile(..) => ByteStream,
            Endpoint::AppendFile(..) => ByteStream,
            Endpoint::Random => ByteStream,
            Endpoint::Zero => ByteStream,
            Endpoint::WriteSplitoff { read, write } => {
                match (read.provides_socket_type(), write.provides_socket_type()) {
                    (ByteStream, ByteStream) => ByteStream,
                    (Datarams, Datarams) => Datarams,
                    _ => {
                        debug!("Incompatible WriteSplitoff socket types: datagram and bytestream");
                        Datarams
                    }
                }
            }
        }
    }
}

impl Overlay {
    pub(super) fn provides_socket_type(&self) -> Option<SocketType> {
        use SocketType::{ByteStream, Datarams};
        Some(match self {
            Overlay::WsUpgrade { .. } => ByteStream,
            Overlay::WsFramer { .. } => Datarams,
            Overlay::StreamChunks => Datarams,
            Overlay::LineChunks => Datarams,
            Overlay::TlsClient { .. } => ByteStream,
            Overlay::WsAccept {} => ByteStream,
            Overlay::Log { datagram_mode } => {
                if *datagram_mode {
                    Datarams
                } else {
                    ByteStream
                }
            }
            Overlay::WsClient => Datarams,
            Overlay::WsServer => Datarams,
            Overlay::ReadChunkLimiter => ByteStream,
            Overlay::WriteChunkLimiter => ByteStream,
            Overlay::WriteBuffer => ByteStream,
            Overlay::LengthPrefixedChunks => Datarams,
            Overlay::SimpleReuser => Datarams,
            Overlay::WriteSplitoff => return None,
        })
    }
}

impl SpecifierStack {
    /// Expected to emit multiple connections in parallel
    pub(super) fn is_multiconn(&self, opts: &WebsocatArgs) -> bool {
        let mut multiconn = match self.innermost {
            Endpoint::TcpConnectByEarlyHostname { .. } => false,
            Endpoint::TcpConnectByLateHostname { .. } => false,
            Endpoint::TcpConnectByIp(..) => false,
            Endpoint::TcpListen(..) => !opts.oneshot,
            Endpoint::TcpListenFd(..) => !opts.oneshot,
            Endpoint::TcpListenFdNamed(..) => !opts.oneshot,
            Endpoint::WsUrl(..) => false,
            Endpoint::WssUrl(..) => false,
            Endpoint::WsListen(..) => !opts.oneshot,
            Endpoint::Stdio => false,
            Endpoint::UdpConnect(..) => false,
            Endpoint::UdpBind(..) => false,
            Endpoint::UdpFd(_) => false,
            Endpoint::UdpFdNamed(_) => false,
            Endpoint::UdpServer(..) => !opts.oneshot,
            Endpoint::UdpServerFd(_) => !opts.oneshot,
            Endpoint::UdpServerFdNamed(_) => !opts.oneshot,
            Endpoint::Exec(..) => false,
            Endpoint::Cmd(..) => false,
            Endpoint::DummyStream => false,
            Endpoint::DummyDatagrams => false,
            Endpoint::Literal(_) => false,
            Endpoint::LiteralBase64(_) => false,
            Endpoint::UnixConnect(..) => false,
            Endpoint::UnixListen(..) => !opts.oneshot,
            Endpoint::AbstractConnect(..) => false,
            Endpoint::AbstractListen(..) => !opts.oneshot,
            Endpoint::UnixListenFd(_) => !opts.oneshot,
            Endpoint::UnixListenFdNamed(_) => !opts.oneshot,
            Endpoint::AsyncFd(_) => false,
            Endpoint::SeqpacketConnect(..) => false,
            Endpoint::SeqpacketListen(..) => !opts.oneshot,
            Endpoint::AbstractSeqpacketConnect(..) => false,
            Endpoint::AbstractSeqpacketListen(..) => !opts.oneshot,
            Endpoint::SeqpacketListenFd(..) => !opts.oneshot,
            Endpoint::SeqpacketListenFdNamed(..) => !opts.oneshot,
            Endpoint::MockStreamSocket(..) => false,
            Endpoint::RegistryStreamListen(..) => !opts.oneshot,
            Endpoint::RegistryStreamConnect(..) => false,
            Endpoint::SimpleReuserEndpoint(..) => false,
            Endpoint::ReadFile(..) => false,
            Endpoint::WriteFile(..) => false,
            Endpoint::AppendFile(..) => false,
            Endpoint::Random => false,
            Endpoint::Zero => false,
            Endpoint::WriteSplitoff { .. } => false,
        };

        for x in &self.overlays {
            match x {
                Overlay::WsUpgrade { .. } => {}
                Overlay::WsAccept { .. } => {}
                Overlay::WsFramer { .. } => {}
                Overlay::WsClient => {}
                Overlay::WsServer => {}
                Overlay::TlsClient { .. } => {}
                Overlay::StreamChunks => {}
                Overlay::LineChunks => {}
                Overlay::LengthPrefixedChunks => {}
                Overlay::Log { .. } => {}
                Overlay::ReadChunkLimiter => {}
                Overlay::WriteChunkLimiter => {}
                Overlay::WriteBuffer => {}
                Overlay::SimpleReuser => multiconn = false,
                Overlay::WriteSplitoff => multiconn = false,
            }
        }

        multiconn
    }

    /// Some specifier or overlay does not like reentrant usage, e.g. stdio: or appendfile: may be chaotic.
    pub(super) fn prefers_being_single(&self, opts: &WebsocatArgs) -> bool {
        let mut singler = match self.innermost {
            Endpoint::TcpConnectByEarlyHostname { .. } => false,
            Endpoint::TcpConnectByLateHostname { .. } => false,
            Endpoint::TcpConnectByIp(..) => false,
            Endpoint::TcpListen(..) => false,
            Endpoint::TcpListenFd(..) => false,
            Endpoint::TcpListenFdNamed(..) => false,
            Endpoint::WsUrl(..) => false,
            Endpoint::WssUrl(..) => false,
            Endpoint::WsListen(..) => false,
            Endpoint::Stdio => true,
            Endpoint::UdpConnect(..) => false,
            Endpoint::UdpBind(..) => true,
            Endpoint::UdpFd(_) => true,
            Endpoint::UdpFdNamed(_) => true,
            Endpoint::UdpServer(..) => false,
            Endpoint::UdpServerFd(_) => false,
            Endpoint::UdpServerFdNamed(_) => false,
            Endpoint::Exec(..) => false,
            Endpoint::Cmd(..) => false,
            Endpoint::DummyStream => false,
            Endpoint::DummyDatagrams => false,
            Endpoint::Literal(_) => false,
            Endpoint::LiteralBase64(_) => false,
            Endpoint::UnixConnect(..) => false,
            Endpoint::UnixListen(..) => false,
            Endpoint::AbstractConnect(..) => false,
            Endpoint::AbstractListen(..) => false,
            Endpoint::UnixListenFd(_) => false,
            Endpoint::UnixListenFdNamed(_) => false,
            Endpoint::AsyncFd(_) => true,
            Endpoint::SeqpacketConnect(..) => false,
            Endpoint::SeqpacketListen(..) => false,
            Endpoint::AbstractSeqpacketConnect(..) => false,
            Endpoint::AbstractSeqpacketListen(..) => false,
            Endpoint::SeqpacketListenFd(..) => false,
            Endpoint::SeqpacketListenFdNamed(..) => false,
            Endpoint::MockStreamSocket(..) => false,
            Endpoint::RegistryStreamListen(..) => false,
            Endpoint::RegistryStreamConnect(..) => false,
            Endpoint::SimpleReuserEndpoint(..) => false,
            Endpoint::ReadFile(..) => false,
            Endpoint::WriteFile(..) => !opts.write_file_no_overwrite,
            Endpoint::AppendFile(..) => true,
            Endpoint::Random => false,
            Endpoint::Zero => false,
            Endpoint::WriteSplitoff {
                ref read,
                ref write,
            } => read.prefers_being_single(opts) || write.prefers_being_single(opts),
        };

        for x in &self.overlays {
            match x {
                Overlay::WsUpgrade { .. } => {}
                Overlay::WsAccept { .. } => {}
                Overlay::WsFramer { .. } => {}
                Overlay::WsClient => {}
                Overlay::WsServer => {}
                Overlay::TlsClient { .. } => {}
                Overlay::StreamChunks => {}
                Overlay::LineChunks => {}
                Overlay::LengthPrefixedChunks => {}
                Overlay::Log { .. } => {}
                Overlay::ReadChunkLimiter => {}
                Overlay::WriteChunkLimiter => {}
                Overlay::WriteBuffer => {}
                Overlay::SimpleReuser => singler = false,
                Overlay::WriteSplitoff => {}
            }
        }

        singler
    }
}
