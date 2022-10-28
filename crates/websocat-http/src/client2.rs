use websocat_api::{
    anyhow, async_trait::async_trait, bytes, futures::TryStreamExt, tokio, NodeId, Result, http,
};
use websocat_derive::{WebsocatNode, WebsocatMacro};

#[derive(Debug, derivative::Derivative, WebsocatNode)]
#[websocat_node(official_name = "http-client2")]
#[auto_populate_in_allclasslist]
#[derivative(Clone)]
pub struct HttpClient2 {
    /// Low-level mode: IO object to use for HTTP1 handshake
    /// If unset, use high-level mode and expect `uri` to be set and be a full URI
    inner: Option<NodeId>,

    #[cfg(feature="highlevel")]
    #[websocat_prop(ignore)]
    #[derivative(Clone(clone_with="ignorant_default"))]
    client: tokio::sync::Mutex<Option<hyper::client::Client<hyper::client::connect::HttpConnector, hyper::body::Body>>>,
}

#[allow(unused)]
fn ignorant_default<T : Default>(_x: &T) -> T {
    Default::default()
}

#[async_trait]
impl websocat_api::RunnableNode for HttpClient2 {
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

        let (rqrs_tx, rqrs_rx) = tokio::sync::mpsc::channel::<websocat_api::HttpRequestWithAResponseSlot>(1);

       
        //let body = resp.into_body();

        //let r = websocat_api::Source::Datagrams(Box::pin(body.map_err(|e| e.into())));

        //let (r,w) = io.unwrap().into_inner();
        Ok(websocat_api::Bipipe {
            r: websocat_api::Source::None,
            w: websocat_api::Sink::None,
            //w: websocat_api::Sink::Http(rqrs_tx),
            closing_notification: cn,
        })
            
        
    }
}
