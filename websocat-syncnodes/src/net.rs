
#[derive(Debug,Clone,websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name="sync-tcp",
)]
pub struct TcpConnect {
    /// Address to connect to
    addr: std::net::SocketAddr,
}

impl websocat_api::SyncNode for TcpConnect {
    fn run(
        &self,
        _ctx: websocat_api::RunContext,
        _allow_multiconnect: bool,
        mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static,
    ) -> websocat_api::Result<()> {
        let addr = self.addr;
        std::thread::spawn(move|| -> websocat_api::Result<()> {
            let t = std::net::TcpStream::connect(addr)?;
            let t = websocat_api::sync::ArcReadWrite::new(t);
            closure(websocat_api::sync::Bipipe {
                r: websocat_api::sync::Source::ByteStream(Box::new(t.clone())),
                w: websocat_api::sync::Sink::ByteStream(Box::new(t)),
                closing_notification: None,
            })?;
            Ok(())
        });
        Ok(())
    }
}


#[derive(Debug,Clone,websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name="sync-tcp-listen",
)]
pub struct TcpListen {
    /// Address bind TCP port to
    addr: std::net::SocketAddr,
}

impl websocat_api::SyncNode for TcpListen {
    fn run(
        &self,
        _ctx: websocat_api::RunContext,
        allow_multiconnect: bool,
        mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static,
    ) -> websocat_api::Result<()> {
        let addr = self.addr;
        let l = std::net::TcpListener::bind(addr)?;
        std::thread::spawn(move|| -> websocat_api::Result<()> {
            while let Ok((t,fromaddr)) = l.accept() {
                tracing::info!("Accepted a TCP connection from {}", fromaddr);

                let t = websocat_api::sync::ArcReadWrite::new(t);
                closure(websocat_api::sync::Bipipe {
                    r: websocat_api::sync::Source::ByteStream(Box::new(t.clone())),
                    w: websocat_api::sync::Sink::ByteStream(Box::new(t)),
                    closing_notification: None,
                })?;
                if ! allow_multiconnect {
                    break;
                }
            }
            Ok(())
        });
        Ok(())
    }
}
