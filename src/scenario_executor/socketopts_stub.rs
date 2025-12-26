#![allow(unused_variables)]
use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream, UdpSocket};

pub struct BindOptions {
    pub(crate) bind_before_connecting: Option<SocketAddr>,
    pub(crate) listen_backlog: u32,
}

pub struct TcpStreamOptions {
}

pub struct UdpOptions {

}


#[macro_export]
macro_rules! copy_common_bind_options {
    ($target:ident, $source:ident) => {
    };
}

#[macro_export]
macro_rules! copy_common_tcp_stream_options {
    ($target:ident, $source:ident) => {
      
    };
}


#[macro_export]
macro_rules! copy_common_udp_options {
    ($target:ident, $source:ident) => {
    
    };
}

impl BindOptions {
    pub fn new() -> BindOptions {
        Self {
            bind_before_connecting: None,
            listen_backlog: 1024,
        }
    }

    pub fn setopts_udp(&self, ss: &socket2::Socket, v6: bool) -> std::io::Result<()> {
        Ok(())
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
        stream_opts: &TcpStreamOptions,
    ) -> std::io::Result<TcpStream> {
        TcpStream::connect(addr).await
    }

    pub async fn bind_tcp(&self, addr: SocketAddr) -> std::io::Result<TcpListener> {
        TcpListener::bind(addr).await
    }

    pub async fn bind_udp(&self, addr: SocketAddr) -> std::io::Result<UdpSocket> {
        UdpSocket::bind(addr).await
    }

    pub fn warn_if_options_set(&self) {
    }
}

impl TcpStreamOptions {
    pub fn new() -> TcpStreamOptions {
        Self {
        }
    }

    pub fn apply_socket_opts(&self, s: &TcpStream, v6: bool) -> std::io::Result<()> {
        Ok(())
    }
}


impl UdpOptions {
    pub fn new() -> UdpOptions {
        Self {
        }
    }

    pub fn apply_socket_opts(&self, s: &UdpSocket, v6: bool) -> std::io::Result<()> {
        Ok(())
    }
}
