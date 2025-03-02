use anyhow::bail;
use base64::Engine as _;
use bytes::BytesMut;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use std::sync::Arc;
use tokio_native_tls::native_tls::{Certificate, Protocol, TlsConnector};
use tracing::{debug, debug_span, warn, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{StreamRead, StreamWrite},
    utils1::{ExtractHandleOrFail, SimpleErr, TaskHandleExt2},
};

use super::{
    types::{Handle, StreamSocket, Task},
    utils1::RhResult,
};

//@ Create environment for using TLS clients.
fn tls_client_connector(
    ctx: NativeCallContext,
    opts: Dynamic,
) -> RhResult<Arc<tokio_native_tls::TlsConnector>> {
    debug!("tls_client_connector");
    #[derive(serde::Deserialize)]
    struct TslConnectorOpts {
        min_protocol_version: Option<String>,
        max_protocol_version: Option<String>,
        #[serde(default)]
        root_certificates_pem: Vec<String>,
        #[serde(default)]
        root_certificates_der_base64: Vec<String>,
        #[serde(default)]
        disable_built_in_roots: bool,
        #[serde(default)]
        request_alpns: Vec<String>,
        #[serde(default)]
        danger_accept_invalid_certs: bool,
        #[serde(default)]
        danger_accept_invalid_hostnames: bool,
        #[serde(default)]
        no_sni: bool,
    }
    let opts: TslConnectorOpts = rhai::serde::from_dynamic(&opts)?;
    debug!("options parsed");

    let mut b = TlsConnector::builder();

    let parseproto = |x: &str| -> RhResult<Protocol> {
        Ok(match x {
            "ssl3" => Protocol::Sslv3,
            "tls10" => Protocol::Tlsv10,
            "tls11" => Protocol::Tlsv11,
            "tls12" => Protocol::Tlsv12,
            _ => return Err(ctx.err("Unknown TLS protocol specified")),
        })
    };

    if let Some(ref q) = opts.min_protocol_version {
        b.min_protocol_version(Some(parseproto(q)?));
    }
    if let Some(ref q) = opts.max_protocol_version {
        b.max_protocol_version(Some(parseproto(q)?));
    }
    for q in &opts.root_certificates_pem {
        match Certificate::from_pem(q.as_bytes()) {
            Ok(x) => b.add_root_certificate(x),
            Err(e) => {
                warn!("Failed to parse PEM certificate: {e}");
                return Err(ctx.err("Failed to parse a certificate"));
            }
        };
    }
    for q in &opts.root_certificates_der_base64 {
        match base64::prelude::BASE64_STANDARD.decode(q) {
            Ok(r) => {
                match Certificate::from_der(&r) {
                    Ok(x) => b.add_root_certificate(x),
                    Err(e) => {
                        warn!("Failed to parse DER certificate: {e}");
                        return Err(ctx.err("Failed to parse a certificate"));
                    }
                };
            }
            Err(e) => {
                warn!("Failed to decode base64 for DER certificate: {e}");
                return Err(ctx.err("Failed to decode base64 for DER certificate"));
            }
        }
    }
    if opts.disable_built_in_roots {
        b.disable_built_in_roots(true);
    }
    if !opts.request_alpns.is_empty() {
        #[cfg(feature = "native-tls-alpn")]
        {
            let refs: Vec<&str> = opts.request_alpns.iter().map(|x| &**x).collect();
            b.request_alpns(&refs);
        }
        #[cfg(not(feature = "native-tls-alpn"))]
        {
            return Err(ctx.err("TLS ALPN support is not enabled at compication time."));
        }
    }
    if opts.danger_accept_invalid_certs {
        b.danger_accept_invalid_certs(true);
    }
    if opts.danger_accept_invalid_hostnames {
        b.danger_accept_invalid_hostnames(true);
    }
    if opts.no_sni {
        b.use_sni(false);
    }

    let cx = match b.build() {
        Ok(x) => x,
        Err(e) => {
            warn!("Failed to create TlsConnector: {e}");
            return Err(ctx.err("Failed to create TlsConnector"));
        }
    };

    let cx = tokio_native_tls::TlsConnector::from(cx);

    Ok(Arc::new(cx))
}

//@ Perform TLS handshake using downstream stream-oriented socket, then expose stream-oriented socket interface to upstream that encrypts/decryptes the data.
fn tls_client(
    ctx: NativeCallContext,
    opts: Dynamic,
    connector: Arc<tokio_native_tls::TlsConnector>,
    inner: Handle<StreamSocket>,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let span = debug_span!("tls_client");
    let the_scenario = ctx.get_scenario()?;
    #[derive(serde::Deserialize)]
    struct TslClientOpts {
        domain: String,
    }
    let opts: TslClientOpts = rhai::serde::from_dynamic(&opts)?;
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
        let socket = connector.connect(&domain, io).await?;
        let (r, w) = tokio::io::split(socket);

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
