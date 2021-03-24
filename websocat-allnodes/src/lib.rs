//! Manually maintained registry of all known Websocat nodes

/// Get `ClassRegistrar` with all WebSocat's nodes registered
pub fn all_node_classes() -> websocat_api::ClassRegistrar {
    let mut reg = websocat_api::ClassRegistrar::default();
    reg.register::<websocat_basic::net::Tcp>();
    reg.register::<websocat_basic::net::TcpListen>();
    reg.register::<websocat_basic::io_std::Stdio>();
    reg.register::<websocat_syncnodes::net::TcpConnect>();
    reg.register::<websocat_syncnodes::net::TcpListen>();
    reg.register::<websocat_syncnodes::net::UdpConnect>();
    reg.register::<websocat_syncnodes::net::UdpListen>();
    
    reg.register::<websocat_http::HttpClient>();
    reg.register::<websocat_http::Header>();

    reg.register::<websocat_ioless::Identity>();
    reg.register::<websocat_ioless::Mirror>();
    reg.register::<websocat_ioless::DevNull>();
    reg.register::<websocat_ioless::Split>();
    reg.register::<websocat_ioless::Literal>();
    reg.register::<websocat_ioless::Stream>();
    reg.register::<websocat_ioless::Datagrams>();
    reg.register::<websocat_ioless::reuse::ReuseBroadcast>();
    reg.register::<websocat_ioless::foreachrequest::Spawner>();

    reg.register::<websocat_readline::Readline>();
    reg
}
    
