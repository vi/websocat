
use anyhow::bail;
use bytes::BytesMut;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio_native_tls::native_tls::TlsConnector;
use tracing::{debug, debug_span, Instrument};

use crate::scenario_executor::{scenario::{callback_and_continue, ScenarioAccess}, types::{StreamRead, StreamWrite}, utils::{ExtractHandleOrFail, TaskHandleExt2}};

use super::{
    types::{
        Handle, StreamSocket, Task
    },
    utils::RhResult,
};

fn tls_client(
    ctx: NativeCallContext,
    opts: Dynamic,
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


        let cx = TlsConnector::builder().build()?;
        let cx = tokio_native_tls::TlsConnector::from(cx);

        let Some(domain) = opts.domain else {
            bail!("Connecting without a domain is not supported yet")
        };
        let socket = cx.connect(&domain, io).await?;
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
    engine.register_fn("tls_client", tls_client);
}
