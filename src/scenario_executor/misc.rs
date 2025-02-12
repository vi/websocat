use std::net::SocketAddr;

use rhai::{Engine, FnPtr, NativeCallContext};
use tracing::{debug, debug_span, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite},
    utils1::TaskHandleExt2,
};

use super::{types::Task, utils1::RhResult};

//@ Obtain a stream socket made of stdin and stdout.
//@ This spawns a OS thread to handle interactions with the stdin/stdout and may be inefficient.
fn create_stdio() -> Handle<StreamSocket> {
    StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::stdin()),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::stdout()),
        }),
        close: None,
    }
    .wrap()
}

//@ Perform a DNS lookup of the specified hostname and call a continuation with the list of IPv4 and IPv6 socket addresses
fn lookup_host(
    ctx: NativeCallContext,
    addr: String,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "resolve");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");

    Ok(async move {
        debug!("node started");
        let ips: Vec<SocketAddr> = tokio::net::lookup_host(addr).await?.collect();

        callback_and_continue::<(Vec<SocketAddr>,)>(the_scenario, continuation, (ips,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("create_stdio", create_stdio);
    engine.register_fn("lookup_host", lookup_host);
}
