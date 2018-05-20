// This is an X-Macro.
#[macro_export]
macro_rules! list_of_all_specifier_classes {
    ($your_macro:ident) => {
        $your_macro!($crate::ws_client_peer::WsClientClass);
        $your_macro!($crate::ws_server_peer::WsServerClass);
        
        #[cfg(all(unix, not(feature = "no_unix_stdio")))]
        $your_macro!($crate::stdio_peer::StdioClass);
        #[cfg(not(all(unix, not(feature = "no_unix_stdio"))))]
        $your_macro!($crate::stdio_threaded_peer::ThreadedStdioSubstituteClass);
        
        $your_macro!($crate::net_peer::TcpConnectClass);
        $your_macro!($crate::net_peer::TcpListenClass);
        
        #[cfg(feature = "tokio-process")]
        $your_macro!($crate::process_peer::ShCClass);
        #[cfg(feature = "tokio-process")]
        $your_macro!($crate::process_peer::ExecClass);
        
        $your_macro!($crate::file_peer::ReadFileClass);
        $your_macro!($crate::file_peer::WriteFileClass);
        $your_macro!($crate::file_peer::AppendFileClass);
        
        $your_macro!($crate::connection_reuse_peer::ReuserClass);
        $your_macro!($crate::reconnect_peer::AutoReconnectClass);
        
        $your_macro!($crate::ws_client_peer::WsConnectClass);
        
        $your_macro!($crate::net_peer::UdpConnectClass);
        $your_macro!($crate::net_peer::UdpListenClass);
        
        #[cfg(all(unix, not(feature = "no_unix_stdio")))]
        $your_macro!($crate::stdio_peer::OpenAsyncClass);
        #[cfg(all(unix, not(feature = "no_unix_stdio")))]
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
        
        $your_macro!($crate::line_peer::Packet2LineClass);
        $your_macro!($crate::mirror_peer::MirrorClass);
        $your_macro!($crate::mirror_peer::LiteralReplyClass);
        $your_macro!($crate::trivial_peer::CloggedClass);
        $your_macro!($crate::trivial_peer::LiteralClass);
        $your_macro!($crate::trivial_peer::AssertClass);
        
        
        #[cfg(feature="seqpacket")]
        $your_macro!($crate::unix_peer::SeqpacketConnectClass);
        #[cfg(feature="seqpacket")]
        $your_macro!($crate::unix_peer::SeqpacketListenClass);
        
        /*
        $your_macro!($crate:: :: );
        $your_macro!($crate:: :: );
        $your_macro!($crate:: :: );
        */
    }
}
