
use anyhow::bail;
use bytes::BytesMut;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio_native_tls::native_tls::TlsConnector;
use tracing::{debug, debug_span, warn, Instrument};
use std::sync::Arc;

use crate::scenario_executor::{scenario::{callback_and_continue, ScenarioAccess}, types::{StreamRead, StreamWrite}, utils::{ExtractHandleOrFail, SimpleErr, TaskHandleExt2}};

use super::{
    types::{
        Handle, StreamSocket, Task
    },
    utils::RhResult,
};

fn tls_client_connector(
    ctx: NativeCallContext,
    opts: Dynamic,
) -> RhResult<Arc<tokio_native_tls::TlsConnector>> {
    debug!("tls_client_connector");
    #[derive(serde::Deserialize)]
    struct TslConnectorOpts {
    }
    let _opts: TslConnectorOpts = rhai::serde::from_dynamic(&opts)?;
    debug!("options parsed");

    let cx = match TlsConnector::builder().build() {
        Ok(x) => x,
        Err(e) => {
            warn!("Failed to create TlsConnector: {e}");
            return Err(ctx.err("Failed to create TlsConnector"));
        }
    };
    let cx = tokio_native_tls::TlsConnector::from(cx);

    Ok(Arc::new(cx))
}


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
       domain: Option<String>,
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
            } = inner else {
                bail!("Incomplete underlying socket specified")
            };

        let io = tokio::io::join(r, w.writer);

        let Some(domain) = opts.domain else {
            bail!("Connecting without a domain is not supported yet")
        };
        let socket = connector.connect(&domain, io).await?;
        let (r,w) = tokio::io::split(socket);

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: Box::pin(r),
                prefix: BytesMut::new(),
            }),
            write: Some(StreamWrite {
                writer: Box::pin(w),
            }),
            close: c,
        };
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue(the_scenario, continuation, (h,))
            .await;
        Ok(())
    }.instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("tls_client_connector", tls_client_connector);
    engine.register_fn("tls_client", tls_client);
}
