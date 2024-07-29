use std::{ffi::{OsStr, OsString}, net::SocketAddr, time::Duration};

use crate::scenario_executor::utils::TaskHandleExt2;
use base64::Engine as _;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::{net::TcpStream, process::Command};
use tracing::{debug, debug_span, error, Instrument};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{Handle, StreamRead, StreamSocket, StreamWrite, Task},
};

use super::utils::{HandleExt, RhResult, SimpleErr};


//@ Prepare subprocess, setting up executable name.
fn subprocess_new(program_name: String) -> Handle<Command> {
    Some(Command::new(program_name)).wrap()
}

//@ Prepare subprocess, setting up possibly non-UTF8 executable name 
fn subprocess_new_osstr(program_name: OsString) -> Handle<Command> {
    Some(Command::new(program_name)).wrap()
}

//@ Start child process and interpret its stdin/stdout as a StreamSocket.
fn subprocess(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "subprocess");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct SubprocessOpts {
        program: String, 

        argv: Vec<String>,

        //@ Interpret `argv` as base64-encoded buffers instead of direct strings.
        base64_args: bool,
    }
    let opts: SubprocessOpts = rhai::serde::from_dynamic(&opts)?;
    let opts: SubprocessOpts = rhai::serde::from_dynamic(&opts)?;
    
    let program_name = opts.program;

    debug!(parent: &span, "options parsed");

    Ok(async move {
        debug!("node started");

        let mut c = Command::new(program_name);
        //c.args(args)

        let s : StreamSocket = todo!();
        debug!(s=?s, "connected");
        let h = s.wrap();

        callback_and_continue::<(Handle<StreamSocket>,)>(the_scenario, continuation, (h,)).await;
        Ok(())
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("subprocess_new", subprocess_new);
    engine.register_fn("subprocess_new_osstr", subprocess_new_osstr);
    engine.register_fn("subprocess", subprocess);
}
