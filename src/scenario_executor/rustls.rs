use anyhow::bail;
use bytes::BytesMut;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio_rustls::TlsConnector;

use tracing::{debug, debug_span, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{StreamRead, StreamWrite},
    utils1::{ExtractHandleOrFail, TaskHandleExt2},
};

use super::{
    types::{Handle, StreamSocket, Task},
    utils1::RhResult,
};

//@ doc(hidden)
//@ Create environment for using TLS clients.
fn tls_client_connector(_ctx: NativeCallContext, opts: Dynamic) -> RhResult<Arc<TlsConnector>> {
    debug!("tls_client_connector");
    #[derive(serde::Deserialize)]
    struct Opts {

    }
    let _opts: Opts = rhai::serde::from_dynamic(&opts)?;
    debug!("options parsed");

    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));

    Ok(Arc::new(connector))
}

//@ doc(hidden)
//@ Perform TLS handshake using downstream stream-oriented socket, then expose stream-oriented socket interface to upstream that encrypts/decryptes the data.
fn tls_client(
    ctx: NativeCallContext,
    opts: Dynamic,
    connector: Arc<TlsConnector>,
    inner: Handle<StreamSocket>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("tls_client");
    let the_scenario = ctx.get_scenario()?;
    #[derive(serde::Deserialize)]
    struct Opts {
        domain: String,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");

    Ok(async move {
        let opts = opts;
        debug!("node started");
        let StreamSocket {
            read: Some(r),
            write: Some(w),
            close: c,
            fd,
        } = inner
        else {
            bail!("Incomplete underlying socket specified")
        };

        let io = tokio::io::join(r, w.writer);

        let mut domain = opts.domain;
        if domain.is_empty() {
            domain = "nodomain".to_owned();
        }
        let domain = ServerName::try_from(domain.as_str())?.to_owned();
        let stream = connector.connect(domain, io).await?;

        let (r, w) = tokio::io::split(stream);

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: Box::pin(r),
                prefix: BytesMut::new(),
            }),
            write: Some(StreamWrite {
                writer: Box::pin(w),
            }),
            close: c,
            fd,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("tls_client_connector", tls_client_connector);
    engine.register_fn("tls_client", tls_client);
}
