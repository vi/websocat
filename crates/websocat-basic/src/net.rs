use websocat_api::anyhow::Context;


#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(official_name=".sockaddr", data_only)]
struct SockAddr {
    /// Specify a set of socket addresses.
    /// For clients, it would initiate a race for the first successfull connection.
    /// For servers, it would cause it to listen all of the mentioned ports.
    addrs: Vec<std::net::SocketAddr>,

    /// A port to connect to. Must be combined with `port`.
    port: Option<u16>,

    /// A host to resolve, then to connect to. Must be combined with `host`.
    host: Option<String>,

    /// TCP host and port pair to resolve and connect to.
    hostport: Option<String>,

    /// Resolve hostname to IP once, at start, not every time before the connection
    #[cli="cache-resolved-ip"]
    #[websocat_prop(default=false)]
    cache_resolved_ip: bool,
}

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(official_name = "tcp", prefix = "tcp", validate)]
pub struct Tcp {
    #[websocat_prop(flatten, delegate_array)]
    sockaddr: SockAddr,
}

impl SockAddr {
    #[tracing::instrument(level = "debug", name = "validate", skip(self,only_one_address), err)]
    fn validate(&mut self, only_one_address: bool) -> websocat_api::anyhow::Result<()> {

        if self.port.is_some() != self.host.is_some() {
            websocat_api::anyhow::bail!("`host` and `port` options must be specified together");
        }
        let mut specifiers = 0;
        if !self.addrs.is_empty() {
            specifiers += 1;
        }
        if self.hostport.is_some() {
            specifiers += 1;
        }
        if self.host.is_some() {
            specifiers += 1;
        }
        if specifiers < 1 {
            websocat_api::anyhow::bail!("No socket address specified");
        }
        if specifiers > 1 {
            websocat_api::anyhow::bail!("Specify exactly one of {array of explicit addresses}, {`hostport` property} or {`host`+`port` properties}.");
        }

        if only_one_address && self.addrs.len() > 1 {
            websocat_api::anyhow::bail!("Only one address may be specified here");
        }


        if self.addrs.is_empty() && self.cache_resolved_ip {
            self.addrs = self.resolve_sync( only_one_address)?;
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "resolve", skip(self,only_one_address), err)]
    fn resolve_sync(&self, only_one_address: bool) -> websocat_api::anyhow::Result<Vec<std::net::SocketAddr>> {
        use std::net::ToSocketAddrs;

        let mut addrs: Vec<std::net::SocketAddr>;
        if let Some(hostport) = &self.hostport {
            tracing::debug!("Resolving {}", hostport);
            addrs = hostport
                .to_socket_addrs()
                .with_context(|| format!("Error resolving {}", hostport))?
                .collect();
        } else if let (Some(host), Some(port)) = (&self.host, self.port) {
            tracing::debug!("Resolving {}", host);
            addrs = (&host[..], port)
                .to_socket_addrs()
                .with_context(|| format!("Error resolving {}", host))?
                .collect();
        } else {
            unreachable!()
        }
        if only_one_address && addrs.len() > 1 {
            addrs.resize_with(1, || unreachable!());
            tracing::warn!("Using only one of resolved IP addresses");
        }
        tracing::debug!("Resolved to {:?}", addrs);
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("Failed to resolve hostname ip IP address");
        }
        Ok(addrs)
    }

    #[tracing::instrument(level = "debug", name = "resolve", skip(self,only_one_address), err)]
    async fn resolve_async(&self, only_one_address: bool) -> websocat_api::anyhow::Result<Vec<std::net::SocketAddr>> {
        let mut addrs: Vec<std::net::SocketAddr>;
        if let Some(hostport) = &self.hostport {
            tracing::debug!("Resolving {}", hostport);
            addrs = tokio::net::lookup_host(hostport)
                .await
                .with_context(|| format!("Error resolving {}", hostport))?
                .collect();
        } else if let (Some(host), Some(port)) = (&self.host, self.port) {
            tracing::debug!("Resolving {}", host);
            addrs = tokio::net::lookup_host(format!("{}:0", host))
                .await
                .with_context(|| format!("Error resolving {}", host))?
                .map(|sa| std::net::SocketAddr::new(sa.ip(), port))
                .collect();
        } else {
            unreachable!()
        }
        if only_one_address && addrs.len() > 1 {
            addrs.resize_with(1, || unreachable!());
            tracing::warn!("Using only one of resolved IP addresses");
        }
        tracing::debug!("Resolved to {:?}", addrs);
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("Failed to resolve hostname ip IP address");
        }
        Ok(addrs)
    }
}

impl Tcp {
    fn validate(&mut self) -> websocat_api::anyhow::Result<()> {
        self.sockaddr.validate(cfg!(not(feature="race")))?;
        Ok(())
    }
}

#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for Tcp {
    #[tracing::instrument(level = "debug", name = "Tcp", skip(self,_q,_w), err)]
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _q: websocat_api::RunContext,
        _w: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {
        let mut addrs = &self.sockaddr.addrs;
        let addrs_holder;
        if self.sockaddr.addrs.is_empty() {
            addrs_holder = SockAddr::resolve_async(&self.sockaddr, cfg!(not(feature="race"))).await?;
            addrs = &addrs_holder;
        }
        if addrs.is_empty() {
            websocat_api::anyhow::bail!("No destination address for TCP connection specified");
        }
        if addrs.len() == 1 {
            let addr = addrs[0];
            tracing::debug!("Connecting to {}", addr);
            let c = tokio::net::TcpStream::connect(addr).await?;
            let (r, w) = c.into_split();
            tracing::info!("Connected to {}", addr);
            Ok(websocat_api::Bipipe {
                r: websocat_api::Source::ByteStream(Box::pin(r)),
                w: websocat_api::Sink::ByteStream(Box::pin(w)),
                closing_notification: None,
            })
        } else {
            #[cfg(feature = "race")]
            {
                tracing::debug!(
                    "Setting up a race of trying to connect {} addresses",
                    self.sockaddr.addrs.len()
                );
                let mut reply_rx;
                {
                    let (aborter_tx_, _aborter_rx_) = tokio::sync::broadcast::channel(1);
                    let (reply_tx_, reply_rx_) = tokio::sync::mpsc::channel(1);
                    reply_rx = reply_rx_;

                    for a in addrs.iter() {
                        let a = *a;
                        let mut aborter_rx = aborter_tx_.subscribe();
                        let aborter_tx = aborter_tx_.clone();
                        let reply_tx = reply_tx_.clone();

                        let logger =
                            tracing::debug_span!("racer", addr = tracing::field::display(a));

                        tokio::spawn(async move {
                            tracing::debug!(parent: &logger, "Initiating connection");
                            tokio::select! {
                                _abt = aborter_rx.recv() => {
                                    tracing::debug!(parent: &logger, "Too late, aborting attempt.");
                                },
                                conn = tokio::net::TcpStream::connect(a) => match conn {
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

                if let Some((c, a)) = reply_rx.recv().await {
                    let (r, w) = c.into_split();
                    tracing::info!("Connected to {}", a);
                    Ok(websocat_api::Bipipe {
                        r: websocat_api::Source::ByteStream(Box::pin(r)),
                        w: websocat_api::Sink::ByteStream(Box::pin(w)),
                        closing_notification: None,
                    })
                } else {
                    websocat_api::anyhow::bail!(
                        "All {} connection attempts failed",
                        self.sockaddr.addrs.len()
                    )
                }
            }
            #[cfg(not(feature = "race"))]
            {
                websocat_api::anyhow::bail!("Cannot try connecting to multiple addresses without `websocat-basic/race` Cargo feature enabled")
            }
        }
    }
}

#[derive(Debug, Clone, websocat_derive::WebsocatNode)]
#[websocat_node(official_name = "tcp-listen", prefix = "tcp-listen", validate)]
pub struct TcpListen {
    #[websocat_prop(flatten, delegate_array)]
    sockaddr: SockAddr,
}

impl TcpListen {
    fn validate(&mut self) -> websocat_api::Result<()> {
        self.sockaddr.validate(false)?;
        Ok(())
    }
}



#[websocat_api::async_trait::async_trait]
impl websocat_api::RunnableNode for TcpListen {
    #[tracing::instrument(level = "debug", name = "TcpListen", skip(self,multiconn,_q), err)]
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        _q: websocat_api::RunContext,
        mut multiconn: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {
        /// Thing that call passed though when serving multiple connections - either a direct TcpListener or a channel's receiver when
        /// there are multiple receivers
        /// Balls are "kicked" down the stream into `multiconn.call_me_again_with_this` and return from up as `multiconn.you_are_called_not_the_first_time`.
        enum Ball {
            Sole(tokio::net::TcpListener),
            Multiple(tokio::sync::mpsc::Receiver<tokio::net::TcpStream>),
        }


        let mut ball_ : Option<Ball> = None;
        if let Some(multiconn) = &mut multiconn {
            if let Some(mut ball) = multiconn.you_are_called_not_the_first_time.take() {
                let so : &mut Option<Ball>;
                so = ball.downcast_mut().expect("Unexpected object passed to a restarted TcpListen::run");
                ball_ = Some(so.take().unwrap());
                tracing::debug!("Restored the listening socket from multiconn context");
            } else {
                tracing::debug!("This is the first serving of possible series of incoming connections");
            }
        } else {
            tracing::debug!("No multiconn requested");
        }

        let mut ball = if let Some(x) = ball_ { x } else {
            let mut addrs = &self.sockaddr.addrs;
            let addrs_holder;
            if self.sockaddr.addrs.is_empty() {
                addrs_holder = SockAddr::resolve_async(&self.sockaddr, false).await?;
                addrs = &addrs_holder;
            }
            if addrs.is_empty() {
                websocat_api::anyhow::bail!("No addresses for TCP listen specified");
            }

            if addrs.len() == 1 {
                let ret = tokio::net::TcpListener::bind(addrs[0]).await?;
                tracing::debug!("Bound listening socket to single address {}", addrs[0]);
                Ball::Sole(ret)
            } else {
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                for addr in addrs {
                    let tx = tx.clone();
                    let logger =
                        tracing::debug_span!("listener", addr = tracing::field::display(addr));
                    let addr = addr.clone();
                    let _h = tokio::spawn(async move {
                        async fn listener(logger: websocat_api::tracing::Span, addr: std::net::SocketAddr, tx: tokio::sync::mpsc::Sender<tokio::net::TcpStream>) -> websocat_api::Result<()> {
                            let l = tokio::net::TcpListener::bind(addr).await?;
                            tracing::debug!(parent: &logger, "Spawned a listeter");
                            loop {
                                let (c, inaddr) = l.accept().await?;
                                tracing::info!(parent: &logger,"Incoming connection from {}", inaddr);
                                tx.send(c).await?;
                            }
                        }
                        if let Err(e) = listener(logger, addr, tx).await {
                            tracing::error!("In incoming connections acceptor task: {}", e);
                        }
                    });
                }
                Ball::Multiple(rx)
            }
        };
        let c = match ball {
            Ball::Sole(ref l) => {
                let (c, inaddr) = l.accept().await?;
                tracing::info!("Incoming connection from {}", inaddr);
                c
            }
            Ball::Multiple(ref mut rx) => {
                rx.recv().await.with_context(||format!("Failed to receive a connected socket from mpsc channel"))?
            }
        };

        if let Some(multiconn) = multiconn {
            tracing::debug!("Trigger another session");
            (multiconn.call_me_again_with_this)(Box::new(Some(ball)));
        }

        let (r, w) = c.into_split();
        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::ByteStream(Box::pin(r)),
            w: websocat_api::Sink::ByteStream(Box::pin(w)),
            closing_notification: None,
        })
    }
}
