
#[derive(Debug,Clone,websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name="sync-tcp",
)]
#[auto_populate_in_allclasslist]
pub struct TcpConnect {
    /// Address to connect to
    addr: std::net::SocketAddr,
}

impl websocat_api::SyncNode for TcpConnect {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
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
#[auto_populate_in_allclasslist]
pub struct TcpListen {
    /// Address bind TCP port to
    addr: std::net::SocketAddr,
}

impl websocat_api::SyncNode for TcpListen {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
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

#[derive(Debug,Clone,websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name="sync-udp-connect",
)]
#[auto_populate_in_allclasslist]
pub struct UdpConnect {
    /// Address and port to bind UDP socket to
    bind: Option<std::net::SocketAddr>,
    
    /// Address to connect UDP socket to
    connect: std::net::SocketAddr,
}

impl websocat_api::SyncNode for UdpConnect {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: websocat_api::RunContext,
        _allow_multiconnect: bool,
        mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static,
    ) -> websocat_api::Result<()> {
        let bindaddr = if let Some(x) = self.bind { x } else {
            std::net::SocketAddr::new(match self.connect {
                std::net::SocketAddr::V4(_) => std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                std::net::SocketAddr::V6(_) => std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
            }, 0)
        };
        let u = std::net::UdpSocket::bind(bindaddr)?;

        u.connect(&self.connect)?;

        let u = std::sync::Arc::new(u);
        let u2 = u.clone();

        std::thread::spawn(move|| -> websocat_api::Result<()> {
            closure(websocat_api::sync::Bipipe {
                r: websocat_api::sync::Source::Datagrams(Box::new(move || -> websocat_api::Result<Option<websocat_api::bytes::Bytes>> {
                    let mut buf = websocat_api::bytes::BytesMut::with_capacity(2048);
                    buf.resize(buf.capacity(), 0);
                    let (rcv, from) = u.recv_from(&mut buf)?;
                    tracing::debug!("Received datagram of length {} from {}", rcv, from);
                    buf.resize(rcv, 0);
                    Ok(Some(buf.freeze()))
                })),
                w: websocat_api::sync::Sink::Datagrams(Box::new(move |buf| -> websocat_api::Result<()>{
                    u2.send(&buf)?;
                    Ok(())
                })),
                closing_notification: None,
            })?;
            Ok(())
        });
        Ok(())
    }
}

#[derive(Debug,Clone,websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name="sync-udp-listen",
    validate,
)]
#[auto_populate_in_allclasslist]
pub struct UdpListen {
    /// Address and port to bind UDP socket to
    bind: std::net::SocketAddr,
    
    /// Send to this address instead of last previously seen peer address
    sendto: Option<std::net::SocketAddr>,

    /// Remember the first seen peer address and send packets only there
    latch_to_the_first_seen: Option<bool>,
}

impl UdpListen {
    fn validate(&mut self) -> websocat_api::Result<()> {
        if self.sendto.is_some() && self.latch_to_the_first_seen == Some(true) {
            websocat_api::anyhow::bail!("sendto and latch_to_the_first_seen options are incompatible");
        }
        if self.sendto.is_some() {
            self.latch_to_the_first_seen = Some(true);
        }
        if self.latch_to_the_first_seen.is_none() { self.latch_to_the_first_seen = Some(false); }
        Ok(())
    }
}

impl websocat_api::SyncNode for UdpListen {
    fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _ctx: websocat_api::RunContext,
        _allow_multiconnect: bool,
        mut closure: impl FnMut(websocat_api::sync::Bipipe) -> websocat_api::Result<()> + Send + 'static,
    ) -> websocat_api::Result<()> {
        let u = std::net::UdpSocket::bind(self.bind)?;

        struct Info {
            u: std::net::UdpSocket,
            a: std::sync::RwLock<Option<std::net::SocketAddr>>,
            latch_to_the_first_seen: bool,
        }

        let i = std::sync::Arc::new(Info {
            u,
            a: std::sync::RwLock::new(self.sendto),
            latch_to_the_first_seen: self.latch_to_the_first_seen.unwrap(),
        });
        let i2 = i.clone();

        std::thread::spawn(move|| -> websocat_api::Result<()> {
            let span = tracing::info_span!("SyncUdpRecv");
            let span2 = tracing::info_span!("SyncUdpSend");
            closure(websocat_api::sync::Bipipe {
                r: websocat_api::sync::Source::Datagrams(Box::new(move || -> websocat_api::Result<Option<websocat_api::bytes::Bytes>> {
                    let mut buf = websocat_api::bytes::BytesMut::with_capacity(2048);
                    buf.resize(buf.capacity(), 0);
                    let (rcv, from) = i.u.recv_from(&mut buf)?;
                    tracing::debug!(parent: &span, "Received datagram of length {} from {}", rcv, from);
                    let remembered_address = *i.a.read().unwrap();
                    if remembered_address.is_none() {
                        tracing::info!(parent: &span, "Obtained peer address: {} ", from);
                        *i.a.write().unwrap() = Some(from);
                    } else if remembered_address != Some(from) {
                        if ! i.latch_to_the_first_seen {
                            tracing::info!(parent: &span, "Switching to new peer address: {} ", from);
                            *i.a.write().unwrap() = Some(from);
                        } else {
                            tracing::info!(parent: &span, "Received and proceesed datagram from unexpected address: {}", from);
                        }
                    }
                    buf.resize(rcv, 0);
                    Ok(Some(buf.freeze()))
                })),
                w: websocat_api::sync::Sink::Datagrams(Box::new(move |buf| -> websocat_api::Result<()>{
                    let addr = loop {
                        let addr  = *i2.a.read().unwrap();
                        let addr = if let Some(x) = addr { x } else {
                            tracing::warn!(parent: &span2, "No peer address so far. Waiting for incoming datagrams");
                            std::thread::sleep(std::time::Duration::from_millis(1000));
                            continue;
                        };
                        break addr;
                    };
                    i2.u.send_to(&buf, addr)?;
                    Ok(())
                })),
                closing_notification: None,
            })?;
            Ok(())
        });
        Ok(())
    }
}
