// This is an X-Macro.
#[macro_export]
macro_rules! list_of_all_specifier_classes {
    ($your_macro:ident) => {
        $your_macro!($crate::ws_client_peer::WsClientClass);
        #[cfg(feature = "ssl")]
        $your_macro!($crate::ws_client_peer::WsClientSecureClass);
        $your_macro!($crate::ws_server_peer::WsTcpServerClass);
        $your_macro!($crate::ws_server_peer::WsInetdServerClass);
        $your_macro!($crate::ws_server_peer::WsUnixServerClass);
        $your_macro!($crate::ws_server_peer::WsAbstractUnixServerClass);
        $your_macro!($crate::ws_server_peer::WsServerClass);

        #[cfg(feature = "ssl")]
        $your_macro!($crate::ssl_peer::WssListenClass);

        #[cfg(all(unix, feature = "unix_stdio"))]
        $your_macro!($crate::stdio_peer::StdioClass);
        #[cfg(all(unix, feature = "unix_stdio"))]
        $your_macro!($crate::stdio_peer::InetdClass);
        #[cfg(not(all(unix, feature = "unix_stdio")))]
        $your_macro!($crate::stdio_threaded_peer::ThreadedStdioSubstituteClass);
        #[cfg(not(all(unix, feature = "unix_stdio")))]
        $your_macro!($crate::stdio_threaded_peer::InetdClass);

        $your_macro!($crate::net_peer::TcpConnectClass);
        $your_macro!($crate::net_peer::TcpListenClass);

        #[cfg(feature = "ssl")]
        $your_macro!($crate::ssl_peer::TlsConnectClass);
        #[cfg(feature = "ssl")]
        $your_macro!($crate::ssl_peer::TlsAcceptClass);
        #[cfg(feature = "ssl")]
        $your_macro!($crate::ssl_peer::TlsListenClass);

        #[cfg(feature = "tokio-process")]
        $your_macro!($crate::process_peer::ShCClass);
        #[cfg(feature = "tokio-process")]
        $your_macro!($crate::process_peer::CmdClass);
        #[cfg(feature = "tokio-process")]
        $your_macro!($crate::process_peer::ExecClass);

        $your_macro!($crate::file_peer::ReadFileClass);
        $your_macro!($crate::file_peer::WriteFileClass);
        $your_macro!($crate::file_peer::AppendFileClass);

        $your_macro!($crate::primitive_reuse_peer::ReuserClass);
        $your_macro!($crate::broadcast_reuse_peer::BroadcastReuserClass);
        $your_macro!($crate::reconnect_peer::AutoReconnectClass);

        $your_macro!($crate::ws_client_peer::WsConnectClass);

        $your_macro!($crate::net_peer::UdpConnectClass);
        $your_macro!($crate::net_peer::UdpListenClass);

        #[cfg(all(unix, feature = "unix_stdio"))]
        $your_macro!($crate::stdio_peer::OpenAsyncClass);
        #[cfg(all(unix, feature = "unix_stdio"))]
        $your_macro!($crate::stdio_peer::OpenFdAsyncClass);

        $your_macro!($crate::stdio_threaded_peer::ThreadedStdioClass);

        #[cfg(unix)]
        $your_macro!($crate::unix_peer::UnixConnectClass);
        #[cfg(unix)]
        $your_macro!($crate::unix_peer::UnixListenClass);
        #[cfg(unix)]
        $your_macro!($crate::unix_peer::UnixDgramClass);
        #[cfg(unix)]
        $your_macro!($crate::unix_peer::AbstractConnectClass);
        #[cfg(unix)]
        $your_macro!($crate::unix_peer::AbstractListenClass);
        #[cfg(unix)]
        $your_macro!($crate::unix_peer::AbstractDgramClass);

        $your_macro!($crate::line_peer::Message2LineClass);
        $your_macro!($crate::line_peer::Line2MessageClass);
        $your_macro!($crate::mirror_peer::MirrorClass);
        $your_macro!($crate::mirror_peer::LiteralReplyClass);
        $your_macro!($crate::trivial_peer::CloggedClass);
        $your_macro!($crate::trivial_peer::LiteralClass);
        $your_macro!($crate::trivial_peer::AssertClass);
        $your_macro!($crate::trivial_peer::Assert2Class);

        #[cfg(all(target_os = "linux", feature = "seqpacket"))]
        $your_macro!($crate::unix_peer::unix_seqpacket_peer::SeqpacketConnectClass);
        #[cfg(all(target_os = "linux", feature = "seqpacket"))]
        $your_macro!($crate::unix_peer::unix_seqpacket_peer::SeqpacketListenClass);

        $your_macro!($crate::jsonrpc_peer::JsonRpcClass);

        $your_macro!($crate::proxy_peer::SocksProxyClass);
        $your_macro!($crate::proxy_peer::SocksBindClass);
    };
}
