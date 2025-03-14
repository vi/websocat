use anyhow::bail;
use bytes::BytesMut;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use rustls::{client::danger::ServerCertVerifier, pki_types::ServerName, SignatureScheme};
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

#[derive(Debug)]
struct DummyVerifier;

impl ServerCertVerifier for DummyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

//@ doc(hidden)
//@ Create environment for using TLS clients.
fn tls_client_connector(_ctx: NativeCallContext, opts: Dynamic) -> RhResult<Arc<TlsConnector>> {
    debug!("tls_client_connector");
    #[derive(serde::Deserialize)]
    struct Opts {
        #[serde(default)]
        danger_accept_invalid_certs: bool,
        #[serde(default)]
        danger_accept_invalid_hostnames: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;
    debug!("options parsed");

    let mut root_cert_store = rustls::RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = if opts.danger_accept_invalid_certs && opts.danger_accept_invalid_hostnames {
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(DummyVerifier))
            .with_no_client_auth()
    } else {
        rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth()
    };

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
