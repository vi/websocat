use std::{net::SocketAddr, pin::Pin, task::Poll};

use rand::{RngCore, SeedableRng};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::io::AsyncRead;
use tracing::{debug, debug_span, Instrument};

use crate::scenario_executor::{
    exit_code::{EXIT_CODE_HOSTNAME_LOOKUP_FAIL, EXIT_CODE_HOSTNAME_LOOKUP_NO_IPS},
    scenario::{ScenarioAccess, callback_and_continue},
    types::{Handle, StreamRead, StreamSocket, StreamWrite},
    utils1::TaskHandleExt2,
};

use super::{types::Task, utils1::RhResult};

//@ Obtain a stream socket made of stdin and stdout.
//@ This spawns a OS thread to handle interactions with the stdin/stdout and may be inefficient.
fn stdio_socket() -> Handle<StreamSocket> {
    StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(tokio::io::stdin()),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::stdout()),
        }),
        close: None,
        fd: None,
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
        let ips: Vec<SocketAddr> = tokio::net::lookup_host(addr)
            .await
            .inspect_err(|_| the_scenario.exit_code.set(EXIT_CODE_HOSTNAME_LOOKUP_FAIL))?
            .collect();

        if ips.is_empty() {
            the_scenario.exit_code.set(EXIT_CODE_HOSTNAME_LOOKUP_NO_IPS);            
        }

        callback_and_continue::<(Vec<SocketAddr>,)>(the_scenario, continuation, (ips,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

struct RandomReader<R>(R);

impl<R: RngCore + Unpin> AsyncRead for RandomReader<R> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let b = buf.initialize_unfilled();

        this.0.fill_bytes(b);

        let n = b.len();
        buf.advance(n);

        return Poll::Ready(Ok(()));
    }
}

//@ Create a StreamSocket that reads random bytes (affected by --random-seed) and ignores writes
fn random_socket(ctx: NativeCallContext, opts: Dynamic) -> RhResult<Handle<StreamSocket>> {
    let the_scenario = ctx.get_scenario()?;
    #[derive(serde::Deserialize)]
    struct Opts {
        //@ Use small, less secure RNG instead of slower secure one.
        #[serde(default)]
        fast: bool,
    }
    let opts: Opts = rhai::serde::from_dynamic(&opts)?;

    debug!("random_socket: options parsed");

    let r: Pin<Box<dyn AsyncRead + Send + 'static>> = if !opts.fast {
        let rng = rand_chacha::ChaCha12Rng::from_rng(&mut the_scenario.prng.lock().unwrap());
        Box::pin(RandomReader(rng))
    } else {
        let rng = rand_pcg::Pcg64::from_rng(&mut the_scenario.prng.lock().unwrap());
        Box::pin(RandomReader(rng))
    };

    let w = Box::pin(tokio::io::empty());

    let s = StreamSocket {
        read: Some(StreamRead {
            reader: r,
            prefix: Default::default(),
        }),
        write: Some(StreamWrite { writer: w }),
        close: None,
        fd: None,
    };

    let h = s.wrap();
    Ok(h)
}

struct ZeroReader;

impl AsyncRead for ZeroReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let b = buf.initialize_unfilled();
        let n = b.len();
        buf.advance(n);

        return Poll::Ready(Ok(()));
    }
}

//@ Create a StreamSocket that reads zero bytes and ignores writes
fn zero_socket() -> Handle<StreamSocket> {
    let s = StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(ZeroReader),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(tokio::io::empty()),
        }),
        close: None,
        fd: None,
    };

    let h = s.wrap();
    h
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("stdio_socket", stdio_socket);
    engine.register_fn("lookup_host", lookup_host);
    engine.register_fn("random_socket", random_socket);
    engine.register_fn("zero_socket", zero_socket);
}
