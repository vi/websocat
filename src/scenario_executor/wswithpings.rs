use std::{ops::Range, task::Poll};

use bytes::BytesMut;
use pin_project::pin_project;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rhai::{Dynamic, Engine, NativeCallContext};
use tinyvec::ArrayVec;
use tokio::io::ReadBuf;
use tracing::{debug, debug_span, trace, warn, Span};
use websocket_sans_io::{
    FrameInfo, Opcode, WebsocketFrameDecoder, WebsocketFrameEncoder, MAX_HEADER_LENGTH,
};

use crate::scenario_executor::{utils::{ExtractHandleOrFail, SimpleErr}, wsframer::{WsDecoder, WsEncoder}};

use super::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramSocket, DatagramWrite, Handle, PacketRead, PacketReadResult, PacketWrite, StreamRead, StreamSocket, StreamWrite
    },
    utils::RhResult,
};

fn ws_wrap(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("ws_wrap");
    #[derive(serde::Deserialize)]
    struct WsDecoderOpts {
        client: bool,
        #[serde(default)]
        ignore_masks: bool,
        #[serde(default)]
        no_flush_after_each_message: bool,
    }
    let opts: WsDecoderOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let StreamSocket { read, write, close } = inner;

    let (Some(inner_read), Some(inner_write)) = (read, write) else {
        return Err(ctx.err("Incomplete stream socket"));
    };

    let (require_masked, require_unmasked) = if opts.ignore_masks {
        (false, false)
    } else {
        if opts.client {
            (false, true)
        } else {
            (true, false)
        }
    };

    let d = WsDecoder::new(
        span.clone(),
        inner_read,
        require_masked,
        require_unmasked,
    );
    let dr = DatagramRead { src: Box::pin(d) };

    let e = WsEncoder::new(
        span.clone(),
        opts.client,
        !opts.no_flush_after_each_message,
        inner_write,
    );
    let dw = DatagramWrite { snk: Box::pin(e) };

    let x = DatagramSocket { read: Some(dr), write: Some(dw), close };

    debug!(parent: &span, w=?x, "wrapped");
    Ok(x.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("ws_wrap", ws_wrap);
}
