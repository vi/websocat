use std::net::SocketAddr;

use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, field, Instrument};
use crate::utils::TaskHandleExt;

use crate::{
    scenario::{callback_and_continue, Anyhow2EvalAltResult, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

fn connect_tcp(ctx: NativeCallContext, opts: Dynamic, continuation: FnPtr) -> Result<Handle<Task>, Box<EvalAltResult>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "connect_tcp", addr = field::Empty);
    let the_scenario = ctx.get_scenario().tbar()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct TcpOpts {
        addr: SocketAddr,
    }
    let opts: TcpOpts = rhai::serde::from_dynamic(&opts)?;
    span.record("addr", field::display(opts.addr));
    debug!(parent: &span, "options parsed");

    Ok(async move {
        debug!(parent: &span, "node started");
        let t = tokio::net::TcpStream::connect(opts.addr).await;
        let t = match t {
            Ok(t) => t,
            Err(e) => {
                debug!(parent: &span, error=%e, "connect failed");
                return;
            }
        };
        let (r, w) = t.into_split();
        let (r, w) = (Box::pin(r), Box::pin(w));

        let s = StreamSocket {
            read: Some(StreamRead {
                reader: r,
                prefix: Default::default(),
            }),
            write: Some(StreamWrite { writer: w }),
            close: None,
        };
        debug!(parent: &span, s=?s, "connected");
        let h = s.wrap();

        callback_and_continue(the_scenario, continuation, (h,)).instrument(span).await;
    }
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("connect_tcp", connect_tcp);
}
