use std::net::{IpAddr, SocketAddr};

use http::Uri;

use crate::cli::WebsocatArgs;

use super::{scenarioprinter::ScenarioPrinter, types::{CopyingType, Endpoint, SpecifierStack, WebsocatInvocation}};



impl WebsocatInvocation {
    pub fn get_copying_type(&self) -> CopyingType {
        match (self.left.get_copying_type(), self.right.get_copying_type()) {
            (CopyingType::ByteStream, CopyingType::ByteStream) => CopyingType::ByteStream,
            (CopyingType::Datarams, CopyingType::Datarams) => CopyingType::Datarams,
            _ => panic!("Incompatible types encountered: bytestream-oriented and datagram-oriented"),
        }
    }
}

impl SpecifierStack {
    fn get_copying_type(&self) -> CopyingType {
        assert!(self.overlays.is_empty());
        self.innermost.get_copying_type()
    }
}

impl Endpoint {
    fn get_copying_type(&self) -> CopyingType {
        match self {
            Endpoint::TcpConnectByIp(_) => CopyingType::ByteStream,
            Endpoint::TcpListen(_) => CopyingType::ByteStream,
            Endpoint::WsUrl(_) => CopyingType::Datarams,
            Endpoint::WssUrl(_) => CopyingType::Datarams,
            Endpoint::Stdio => CopyingType::ByteStream,
            Endpoint::UdpConnect(_) => CopyingType::Datarams,
            Endpoint::UdpBind(_) => CopyingType::Datarams,
        }
    }
}
