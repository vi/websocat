use websocat_api::anyhow::Context;


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(
    official_name = "tcp",
    prefix="tcp",
    validate,
)]
pub struct Tcp {
    /// Destination IP and port to where TCP connection should be established
    /// If multiple is specified, they are tried in parallel and the first one who gets though wins.
    addrs : Vec<std::net::SocketAddr>,

    /// TCP port to connect to. Must be combined with `port`.
    port: Option<u16>,

    /// TCP host to resolve, then to connect to. Must be combined with `host`.
    host: Option<String>,

    /// TCP host and port pair to resolve and connect to.
    hostport: Option<String>,

    /// Resolve hostname to IP once, at start, not every time before the connection
    cache_resolved_ip: Option<bool>,
}

impl Tcp {
    fn validate(&mut self) -> websocat_api::anyhow::Result<()> {
        if self.port.is_some() != self.host.is_some() {
            websocat_api::anyhow::bail!("`host` and `port` options must be specified together");
        }
        let mut specifiers = 0;
        if ! self.addrs.is_empty() { specifiers += 1; }
        if self.hostport.is_some() { specifiers += 1; }
        if self.host.is_some() { specifiers += 1; }
        if specifiers < 1 {
            websocat_api::anyhow::bail!("No destination address for TCP connection specified");
        }
        if specifiers > 1 {
            websocat_api::anyhow::bail!("Specify exactly one of {array of explicit addresses}, {`hostport` property} or {`host`+`port` properties}.");
        }
        if self.addrs.len() > 1 {
            #[cfg(not(feature = "race"))] {
                websocat_api::anyhow::bail!("Cannot try connecting to multiple addresses without `websocat-basic/race` Cargo feature enabled")
            }
        }

        if self.addrs.is_empty() && self.cache_resolved_ip==Some(true) {
            self.addrs = self.resolve()?;
        }
        
        Ok(())
    }

    #[tracing::instrument(level="debug", name="resovle", skip(self), err)]
    fn resolve(&self) -> websocat_api::anyhow::Result<std::vec::Vec<std::net::SocketAddr>> {
        use std::net::ToSocketAddrs;
       
        let addrs : Vec<std::net::SocketAddr>;
        if let Some(hostport) = &self.hostport {
            tracing::debug!("Resolving {}", hostport);
            addrs = hostport.to_socket_addrs().with_context(||format!("Error resolving {}", hostport))?.collect();
        } else
        if let (Some(host),Some(port)) = (&self.host,&self.port) {
            tracing::debug!("Resolving {}", host);
            addrs = (&**host,*port).to_socket_addrs().with_context(||format!("Error resolving {}", host))?.collect();
        } else {
            unreachable!()
        }
        #[cfg(not(feature = "race"))] {
            if addrs.len() > 1 {
                addrs.resize_with(1,||unreachable!());
                tracing::warn!("Using only one of resolved IP addresses due to missing  `websocat-basic/race` Cargo feature when compilation");
            }
        }
        tracing::debug!("Resolved to {:?}", addrs);
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("Failed to resolve hostname ip IP address");
        }
        Ok(addrs)
    } 

    #[tracing::instrument(level="debug", name="resovle", skip(self), err)]
    async fn resolve_async(&self) -> websocat_api::anyhow::Result<std::vec::Vec<std::net::SocketAddr>> {
        let addrs : Vec<std::net::SocketAddr>;
        if let Some(hostport) = &self.hostport {
            tracing::debug!("Resolving {}", hostport);
            addrs = tokio::net::lookup_host(hostport).await.with_context(||format!("Error resolving {}", hostport))?.collect();
        } else
        if let (Some(host),Some(port)) = (&self.host,&self.port) {
            tracing::debug!("Resolving {}", host);
            addrs = tokio::net::lookup_host(format!("{}:0", host))
                .await.with_context(||format!("Error resolving {}", host))?
                .map(|sa| std::net::SocketAddr::new(sa.ip(), *port))
                .collect();
        } else {
            unreachable!()
        }
        #[cfg(not(feature = "race"))] {
            if addrs.len() > 1 {
                addrs.resize_with(1,||unreachable!());
                tracing::warn!("Using only one of resolved IP addresses due to missing  `websocat-basic/race` Cargo feature when compilation");
            }
        }
        tracing::debug!("Resolved to {:?}", addrs);
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("Failed to resolve hostname ip IP address");
        }
        Ok(addrs)
    } 
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::Node for Tcp {
    #[tracing::instrument(level="debug", name="Tcp", skip(self), err)]
    async fn run(&self, _: websocat_api::RunContext, _: Option<&mut websocat_api::IWantToServeAnotherConnection>) -> websocat_api::Result<websocat_api::Bipipe> {
        let mut addrs = &self.addrs;
        let addrs_holder;
        if self.addrs.is_empty() {
            addrs_holder = self.resolve_async().await?;
            addrs = &addrs_holder;
        }
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("No destination address for TCP connection specified");
        }
        if addrs.len() == 1 {
            let addr = self.addrs[0];
            tracing::debug!("Connecting to {}", addr);
            let c = tokio::net::TcpStream::connect(addr).await?;
            let (r,w) = c.into_split();
            tracing::info!("Connected to {}", addr);
            Ok(websocat_api::Bipipe {
                r : websocat_api::Source::ByteStream(Box::pin(r)),
                w : websocat_api::Sink::ByteStream(Box::pin(w)),
                closing_notification: None,
            })
        } else {
            #[cfg(feature = "race")] {
                tracing::debug!("Setting up a race of trying to connect {} addresses", self.addrs.len());
                let mut reply_rx;
                {
                    let (aborter_tx_, _aborter_rx_) = tokio::sync::broadcast::channel(1); 
                    let (reply_tx_, reply_rx_) = tokio::sync::mpsc::channel(1); 
                    reply_rx = reply_rx_;

                    for a in addrs.iter() {
                        let a = a.clone();
                        let mut aborter_rx = aborter_tx_.subscribe();
                        let aborter_tx = aborter_tx_.clone();
                        let reply_tx = reply_tx_.clone();

                        let logger = tracing::debug_span!("racer", addr=tracing::field::display(a));

                        tokio::spawn( async move {
                            use futures::FutureExt;
                            tracing::debug!(parent: &logger, "Initiating connection");
                            futures::select! {
                                _abt = aborter_rx.recv().fuse() => {
                                    tracing::debug!(parent: &logger, "Too late, aborting attempt.");
                                },
                                conn = tokio::net::TcpStream::connect(a).fuse() => match conn {
                                    Ok(c) => {
                                        let _ = aborter_tx.send(());
                                        tracing::debug!(parent: &logger, "Connection established");
                                        if reply_tx.send((c, a)).await.is_err() {
                                            tracing::debug!(parent: &logger, "Too late, dropping the connection.");
                                        } 
                                    }
                                    Err(e) => {
                                        tracing::debug!(parent: &logger, "Connection failed: {}", e);
                                    }
                                },
                            }
                        });
                    }
                }
                
                if let Some((c,a)) = reply_rx.recv().await {
                    let (r,w) = c.into_split();
                    tracing::info!("Connected to {}", a);
                    Ok(websocat_api::Bipipe {
                        r : websocat_api::Source::ByteStream(Box::pin(r)),
                        w : websocat_api::Sink::ByteStream(Box::pin(w)),
                        closing_notification: None,
                    })
                } else {
                    websocat_api::anyhow::bail!("All {} connection attempts failed", self.addrs.len())
                }
            }
            #[cfg(not(feature = "race"))] {
                websocat_api::anyhow::bail!("Cannot try connecting to multiple addresses without `websocat-basic/race` Cargo feature enabled")
            }
        }
    }
}
