use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result, http,
};
use websocat_derive::{WebsocatNode, WebsocatMacro};
#[derive(Debug, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-client", validate)]
#[auto_populate_in_allclasslist]
#[derivative(Clone)]
pub struct HttpClient {
    /// Low-level mode: IO object to use for HTTP1 handshake
    /// If unset, use high-level mode and expect `uri` to be set and be a full URI
    inner: Option<NodeId>,

    /// Expect and handle upgrades
    #[websocat_prop(default=false)]
    upgrade: bool,

    /// Immediately return connection, stream bytes into request body.
    #[websocat_prop(default=false)]
    stream_request_body: bool,

    /// Subnode to read request body from
    request_body: Option<NodeId>,

    /// Fully read request body into memory prior to sending it
    #[websocat_prop(default=false)]
    buffer_request_body: bool,

    /// Preallocate this amount of memory for caching request body
    #[websocat_prop(default=1024, min=1, reasonable_max=100_000_000)]
    buffer_request_body_size_hint: i64,

    /// Override HTTP request verb
    method: Option<String>,

    /// Set request content_type to `application/json`
    //#[cli="json"]
    #[websocat_prop(default=false)]
    json: bool, 

    /// Set request content_type to `text/plain`
    //#[cli="json"]
    #[websocat_prop(default=false)]
    textplain: bool, 

    /// Add these headers to HTTP request
    request_headers: Vec<NodeId>,

    /// Request URI
    uri : Option<websocat_api::http::Uri>,

    /// Request WebSocket upgrade from server
    #[websocat_prop(default=false)]
    websocket: bool,

    #[cfg(feature="highlevel")]
    #[websocat_prop(ignore)]
    #[derivative(Clone(clone_with="ignorant_default"))]
    client: tokio::sync::Mutex<Option<hyper::client::Client<hyper::client::connect::HttpConnector, hyper::body::Body>>>,
}

#[allow(unused)]
fn ignorant_default<T : Default>(_x: &T) -> T {
    Default::default()
}

impl HttpClient {
    fn validate(&mut self) -> Result<()> {
        let (uri_is_websocket, _uri_is_wss) = match self.uri {
            Some(ref u) if u.scheme_str() == Some("ws") => (true, false),
            Some(ref u) if u.scheme_str() == Some("wss") => (true, true),
            _ => (false, false),
        };

        if uri_is_websocket {
            self.websocket = true;

            let mut parts = self.uri.take().unwrap().into_parts();
            match parts.scheme.as_ref().unwrap().as_str().to_ascii_lowercase().as_str() {
                "ws" => parts.scheme = Some(http::uri::Scheme::HTTP),
                "wss" => parts.scheme = Some(http::uri::Scheme::HTTPS),
                _ => (),
            }
            self.uri = Some(http::Uri::from_parts(parts).unwrap());
        }

        if self.stream_request_body {
            if self.upgrade {
                anyhow::bail!(
                    "Cannot set both `upgrade` and `stream_request_body` options at the same time"
                );
            }
            if self.request_body.is_some() {
                anyhow::bail!(
                    "Cannot set both `body` and `stream_request_body` options at the same time"
                );
            }
        }

        if self.buffer_request_body
            && (self.request_body.is_none() && !self.stream_request_body)
        {
            anyhow::bail!("buffer_request_body option is meaningless withouth stream_request_body or request_body options");
        }

        if !self.buffer_request_body  && self.buffer_request_body_size_hint != 1024  {
            anyhow::bail!("buffer_request_body_size_hint option is meaningless withouth buffer_request_body option");
        }

        if let Some(ref verb) = self.method {
            let _ = hyper::Method::from_bytes(verb.as_bytes())?;
        }

        if self.textplain && self.json {
            anyhow::bail!("Cannot set both textplain and options to true");
        }

        if self.stream_request_body && self.websocket {
            anyhow::bail!("stream_request_body and websocket options are incompatible");
        }

        if self.websocket {
            self.upgrade = true;
        }

        if self.inner.is_none() {
            // high-level mode
            if let Some(ref uri) = self.uri {
                if uri.authority().is_none() {
                    anyhow::bail!("URI must contain an authority unless `inner` property is set");
                }
            } else {
                anyhow::bail!("Must set either `uri` or `inner` properties");
            }

            #[cfg(not(feature="highlevel"))] {
                anyhow::bail!("`inner` properly is required, as high-level HTTP support is not enabled during compilation");
            }
        }

        Ok(())
    }
}



impl HttpClient {
    fn get_request(&self, body: hyper::Body, ctx: &websocat_api::RunContext) -> Result<(hyper::Request<hyper::Body>, Option<[u8;16]>)> {
        let mut rq = hyper::Request::new(body);
        let mut thekey = None;

        if self.websocket  {
            let r: [u8; 16] = rand::random();
            let key = base64::encode(&r);
            thekey = Some(r);

            rq.headers_mut().insert(hyper::header::CONNECTION, "Upgrade".parse().unwrap());
            rq.headers_mut().insert(hyper::header::UPGRADE, "websocket".parse().unwrap());
            rq.headers_mut().insert(hyper::header::SEC_WEBSOCKET_VERSION, "13".parse().unwrap());
            rq.headers_mut().insert(hyper::header::SEC_WEBSOCKET_KEY, key.parse().unwrap());
        }

        if let Some(ref verb) = self.method {
            *rq.method_mut() = hyper::Method::from_bytes(verb.as_bytes())?;
        } else if self.request_supposed_to_contain_body() {
            *rq.method_mut() = hyper::Method::POST;
        }
        if self.json {
            rq.headers_mut().insert(hyper::header::CONTENT_TYPE, "application/json".parse().unwrap());
        }
        if self.textplain {
            rq.headers_mut().insert(hyper::header::CONTENT_TYPE, "text/plain".parse().unwrap());
        }

        for h in &self.request_headers {
            use websocat_api::PropertyValue;
            let h = &*ctx.nodes[h];
            if let (Some(PropertyValue::Stringy(n)), Some(PropertyValue::Stringy(v))) = (h.get_property("n"), h.get_property("v")) {
                rq.headers_mut().insert(
                    hyper::header::HeaderName::from_bytes(n.as_bytes())?, 
                    hyper::header::HeaderValue::from_bytes(v.as_bytes())?,
                );
            } else {
                anyhow::bail!("http-client's array elements must be `header` nodes");
            }
        }

        if let Some(ref uri) = self.uri {
            *rq.uri_mut() = uri.clone();
        }

        Ok((rq, thekey))
    }

    fn handle_response(&self, resp: &hyper::Response<hyper::Body>, wskey: Option<[u8; 16]>) -> Result<()> {
        tracing::debug!("Response status: {}", resp.status());
        for h in resp.headers() {
            tracing::debug!("Response header: {}={:?}", h.0, h.1);
        }
        if let Some(key) = wskey {
            let hh = resp.headers();
            if let (Some(c), Some(u), Some(a)) = (hh.get(hyper::header::CONNECTION), hh.get(hyper::header::UPGRADE), hh.get(hyper::header::SEC_WEBSOCKET_ACCEPT)) {
                if ! c.as_bytes().eq_ignore_ascii_case(b"upgrade") {
                    anyhow::bail!("http-client is in websocket mode `Connection:` is not `upgrade`");
                }
                if ! u.as_bytes().eq_ignore_ascii_case(b"websocket") {
                    anyhow::bail!("http-client is in websocket mode `Upgrade:` is not `websocket`");
                }
                let accept = crate::util::derive_websocket_accept_key(base64::encode(key).as_bytes());
                if accept != String::from_utf8_lossy(a.as_bytes()) {
                    anyhow::bail!("Sec-Websocket-Accept key mismatch: expected {}, got {:?}", accept, a);
                }
            } else {
                anyhow::bail!("http-client is in websocket mode and some of the three WebSocket response headers are not found");
            }
            tracing::debug!("WebSocket client response verification finished");
        }
        Ok(())
    }

    fn request_supposed_to_contain_body(&self) -> bool {
        self.stream_request_body || self.request_body.is_some()
    }
}

#[async_trait]
impl websocat_api::RunnableNode for HttpClient {
    async fn run(
        self: std::pin::Pin<std::sync::Arc<Self>>,
        ctx: websocat_api::RunContext,
        multiconn: Option<websocat_api::ServerModeContext>,
    ) -> websocat_api::Result<websocat_api::Bipipe> {
        let mut io = None;
        let mut cn = None;
        if let Some(inner) = self.inner {
            let io_ = ctx.nodes[inner].clone().upgrade()?.run(ctx.clone(), multiconn).await?;
            cn = io_.closing_notification;
            io = Some(match (io_.r, io_.w) {
                (websocat_api::Source::ByteStream(r), websocat_api::Sink::ByteStream(w)) => {
                    readwrite::ReadWriteTokio::new(r, w)
                }
                _ => {
                    anyhow::bail!("HTTP client requires a bytestream-based inner node");
                }
            });
        }

        enum ClientVariant {
            Lowlevel(hyper::client::conn::SendRequest<hyper::Body>),
            #[cfg(feature="highlevel")]
            Plain(hyper::Client<hyper::client::HttpConnector>),
        }

        let client : ClientVariant; 
        
        if io.is_some() {
            // low-level mode
            let http_client_builder = hyper::client::conn::Builder::new().handshake::<_, hyper::Body>(io.take().unwrap());
            let (send_request, conn) = http_client_builder.await?;
            let _h = tokio::spawn(conn /* .without_shutdown() */);
            client = ClientVariant::Lowlevel(send_request);
        } else {
            #[cfg(feature="highlevel")] {
                let http_client = {
                    let mut lock = self.client.lock().await;
                    if let Some(ref c) = *lock {
                        c.clone()
                    } else {
                        let c = hyper::client::Client::builder().build_http();
                        *lock = Some(c.clone());
                        c
                    }
                };
    
                client = ClientVariant::Plain(http_client);
            }
            
            #[cfg(not(feature="highlevel"))] {
                unreachable!()
            }
        }

       

        if self.stream_request_body {
            let (response_tx, response_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);
            let w: websocat_api::Sink = if !self.buffer_request_body {
                // Chunked request body
                let (sender, request_body) = hyper::Body::channel();
                let sink = crate::util::body_sink(sender);

                tokio::spawn(async move {
                    let try_block = async move {
                        let rq = self.get_request(request_body, &ctx)?.0;
                        let resp = match client {
                            ClientVariant::Lowlevel(mut send_request) => send_request.send_request(rq).await?,
                            #[cfg(feature="highlevel")]
                            ClientVariant::Plain(http_client) => http_client.request(rq).await?,
                        };
                        self.handle_response(&resp, None)?;
                        let mut body = resp.into_body();
                        use futures::stream::StreamExt;
                        while let Some(buf) = body.next().await {
                            response_tx.send(buf?).await?;
                        }
                        tracing::debug!("Finished sending streamed response");
                        Ok::<_, anyhow::Error>(())
                    };
                    if let Err(e) = try_block.await {
                        tracing::error!("streamed-http-client error: {}", e);
                    }
                });
                websocat_api::Sink::Datagrams(Box::pin(sink))
            } else {
                // Fully buffered request body
                let bufbuf = bytes::BytesMut::with_capacity(
                    self.buffer_request_body_size_hint as usize,
                );
                let (tx, rx) = tokio::sync::oneshot::channel();
                struct SendawayDropper<T>(Option<T>, Option<tokio::sync::oneshot::Sender<T>>);
                impl<T> Drop for SendawayDropper<T> {
                    fn drop(&mut self) {
                        let x: T = self.0.take().unwrap();
                        if let Err(_) = self.1.take().unwrap().send(x) {
                            tracing::error!("Failed to deliver hyper::Body to the appropiate task")
                        } else {
                            tracing::debug!("Finished buffering the hyper::Body")
                        }
                    }
                }

                let bufbufw = SendawayDropper(Some(bufbuf), Some(tx));

                let sink = futures::sink::unfold(
                    bufbufw,
                    move |mut bufbufw, buf: bytes::Bytes| async move {
                        tracing::trace!(
                            "Adding {} bytes chunk to cached HTTP request body",
                            buf.len()
                        );
                        bufbufw.0.as_mut().unwrap().extend(buf);
                        Ok(bufbufw)
                    },
                );
                tokio::spawn(async move {
                    let try_block = async move {
                        let request_buf = rx.await?;
                        let rq = self.get_request(request_buf.freeze().into(), &ctx)?.0;
                        let resp = match client {
                            ClientVariant::Lowlevel(mut send_request) => send_request.send_request(rq).await?,
                            #[cfg(feature="highlevel")]
                            ClientVariant::Plain(http_client) => http_client.request(rq).await?,
                        };
                        self.handle_response(&resp, None)?;
                        let mut body = resp.into_body();
                        use futures::stream::StreamExt;
                        while let Some(buf) = body.next().await {
                            response_tx.send(buf?).await?;
                        }
                        tracing::debug!("Finished sending streamed response");
                        Ok::<_, anyhow::Error>(())
                    };
                    if let Err(e) = try_block.await {
                        tracing::error!("streamed-http-client error: {}", e);
                    }
                });
                websocat_api::Sink::Datagrams(Box::pin(sink))
            };

            let rx = crate::util::body_source(response_rx);
            Ok(websocat_api::Bipipe {
                r: websocat_api::Source::Datagrams(Box::pin(rx)),
                w,
                closing_notification: cn,
            })
        } else {
            // body is not received from upstream in this mode
            let rqbody = if let Some(ref bnid) = self.request_body {
                let bio = ctx.nodes[bnid].clone().upgrade()?.run(ctx.clone(), None).await?;
                drop(bio.w);
                drop(bio.closing_notification);
                if self.buffer_request_body {
                    match bio.r {
                        websocat_api::Source::ByteStream(mut bs) => {
                            use tokio::io::AsyncReadExt;
                            let mut bufbuf = Vec::with_capacity(
                                self.buffer_request_body_size_hint as usize,
                            );
                            bs.read_to_end(&mut bufbuf).await?;
                            bufbuf.into()
                        }
                        websocat_api::Source::Datagrams(x) => {
                            let mut bufbuf = bytes::BytesMut::with_capacity(
                                self.buffer_request_body_size_hint as usize,
                            );
                            use futures::StreamExt;
                            let sink = futures::sink::unfold(
                                &mut bufbuf,
                                move |bufbuf: &mut bytes::BytesMut, buf: bytes::Bytes| async move {
                                    tracing::trace!(
                                        "Adding {} bytes chunk to cached HTTP request body",
                                        buf.len()
                                    );
                                    bufbuf.extend(buf);
                                    Ok::<_,anyhow::Error>(bufbuf)
                                },
                            );
                            x.forward(sink).await?;
                            tracing::debug!("Finished buffering up HTTP request body");
                            bufbuf.freeze().into()
                        }
                        websocat_api::Source::None => {
                            tracing::warn!("Unusable http-client's request_body subnode specifier: null source");
                            hyper::Body::empty() 
                        }
                    }
                } else {
                    // non-buffered body
                    let (sender, body) = hyper::Body::channel();

                    let sink = futures::sink::unfold(
                        sender,
                        move |mut sender, buf: bytes::Bytes| async move {
                            tracing::trace!("Sending {} bytes chunk as HTTP request body", buf.len());
                            sender.send_data(buf).await.map_err(|e| {
                                tracing::error!("Failed sending more HTTP request body: {}", e);
                                e
                            })?;
                            Ok(sender)
                        },
                    );

                    match bio.r {
                        websocat_api::Source::ByteStream(_) => {
                            anyhow::bail!("Use datagram-based subnode for HTTP request body. You may want to wrap it in `[datagrams inner=[...]]` or use buffer_request_body setting.")
                        }
                        websocat_api::Source::Datagrams(x) => {
                            use futures::StreamExt;
                            tokio::spawn(async move {
                                if let Err(e) = x.forward(sink).await {
                                    tracing::error!("Error forwarding chunked http request body from subnode to hyper: {}", e);
                                }
                            });
                            body
                        }
                        websocat_api::Source::None => {
                            tracing::warn!("Unusable http-client's request_body subnode specifier: null source");
                            hyper::Body::empty() 
                        }
                    }
                }
            } else {
                 hyper::Body::empty() 
            };
            let (rq, wskey) = self.get_request(rqbody, &ctx)?;

            let resp = match client {
                ClientVariant::Lowlevel(mut send_request) => send_request.send_request(rq).await?,
                #[cfg(feature="highlevel")]
                ClientVariant::Plain(http_client) => http_client.request(rq).await?,
            };
            self.handle_response(&resp, wskey)?;

            if self.upgrade {
                let upg = hyper::upgrade::on(resp).await?;
                match upg.downcast() {
                    Ok(downc) => {
                        tracing::debug!("Upgraded and recovered low-level inner node");
                        let readbuf = downc.read_buf;
        
                        io = Some(downc.io);
        
                        let (mut r, w) = io.unwrap().into_inner();
        
                        if !readbuf.is_empty() {
                            tracing::debug!("Inserting additional indirection layer due to remaining bytes in the read buffer");
                            r = Box::pin(websocat_api::util::PrependReader(readbuf, r));
                        }
        
                        Ok(websocat_api::Bipipe {
                            r: websocat_api::Source::ByteStream(r),
                            w: websocat_api::Sink::ByteStream(w),
                            closing_notification: cn,
                        })
                    }
                    Err(upg) => {
                        tracing::debug!("Upgraded and turning it to a bytesteam node");
                        let (r,w) = tokio::io::split(upg);
                        Ok(websocat_api::Bipipe {
                            r: websocat_api::Source::ByteStream(Box::pin(r)),
                            w: websocat_api::Sink::ByteStream(Box::pin(w)),
                            closing_notification: cn,
                        })
                    }
                }
            } else {
                let body = resp.into_body();

                let r = websocat_api::Source::Datagrams(Box::pin(body.map_err(|e| e.into())));

                //let (r,w) = io.unwrap().into_inner();
                Ok(websocat_api::Bipipe {
                    r,
                    w: websocat_api::Sink::None,
                    closing_notification: cn,
                })
            }
        }
    }
}

#[derive(Default)]
#[derive(WebsocatMacro)]
#[auto_populate_macro_in_allclasslist]
pub struct AutoLowlevelHttpClient;
impl websocat_api::Macro for AutoLowlevelHttpClient {
    fn official_name(&self) -> String {
        "http".to_owned()
    }
    fn injected_cli_opts(&self) -> Vec<(String, websocat_api::CliOptionDescription)> {
        vec![]
    }

    fn run(&self, strnode: websocat_api::StrNode, _opts: &websocat_api::CliOpts) -> Result<websocat_api::StrNode> {
        let mut uri = Vec::with_capacity(1);

        use websocat_api::stringy::{Ident,StringOrSubnode};

        let mut newnode = websocat_api::StrNode {
            name: Ident("http-client".to_owned()),
            properties: Vec::with_capacity(strnode.properties.len()),
            array: Vec::with_capacity(strnode.array.len()),
            enable_autopopulate: strnode.enable_autopopulate,
        };

        for (prop, val) in strnode.properties {
            match prop.0.as_str() {
                "uri" => match val {
                    StringOrSubnode::Str(x) => uri.push(x),
                    StringOrSubnode::Subnode(_) => anyhow::bail!("Invalid uri property of `http` node"),
                }
                "inner" => anyhow::bail!("`http` does not have `inner` property"),
                _ => {
                    newnode.properties.push((prop,val));
                }
            }
        }

        for val in strnode.array {
            match val {
                StringOrSubnode::Str(x) => uri.push(x),
                StringOrSubnode::Subnode(ref v) => {
                    if v.name.0 == "h" {
                        newnode.array.push(val);
                    } else {
                        anyhow::bail!("`http` node's array mush be either URI or request headers (`h` nodes)")
                    }
                }
            }
        }

        if uri.is_empty() {
            anyhow::bail!("You need to specify URI for `http` node");
        }
        if uri.len() > 1 {
            anyhow::bail!("Too many URIs specified for `http` node");
        }

        let uri = uri.into_iter().next().unwrap();
        let host: websocat_api::bytes::Bytes;
        let port;

        let parsed_uri : http::Uri = String::from_utf8(uri.to_vec())?.parse()?;
        let parsed_uri = parsed_uri.into_parts();
        if let (Some(auth), Some(sch)) = (parsed_uri.authority, parsed_uri.scheme) {
            let h = auth.host();
            host = websocat_api::bytes::Bytes::from(h.as_bytes().to_owned());
            if let Some(p) = auth.port_u16() {
                port = p;
            } else {
                match sch.as_str().to_ascii_lowercase().as_str() {
                    "http" => port = 80,
                    "https" => port = 443,
                    "ws" => port = 80,
                    "wss" => port = 443,
                    _ => anyhow::bail!("Unknown scheme, cannot calculate the port number"),
                }
            }
        } else {
            anyhow::bail!("URI must contain scheme and authority");
        }

        let tcpnode = websocat_api::StrNode {
            name: Ident("tcp".to_owned()),
            properties: vec![
                (Ident("host".to_owned()), StringOrSubnode::Str(host)),
                (Ident("port".to_owned()), StringOrSubnode::Str(format!("{}", port).into())),
            ],
            array: vec![],
            enable_autopopulate: strnode.enable_autopopulate,
        };

        newnode.properties.push((Ident("uri".to_owned()), StringOrSubnode::Str(uri)));
        newnode.properties.push((Ident("inner".to_owned()), StringOrSubnode::Subnode(tcpnode)));

        Ok(newnode)
    }
}
