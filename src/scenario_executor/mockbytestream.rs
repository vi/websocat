use rhai::{Engine, NativeCallContext};

use crate::scenario_executor::utils1::SimpleErr;

use super::{
    types::{Handle, StreamRead, StreamSocket, StreamWrite},
    utils1::RhResult,
};

//@ Create special testing stream socket that emits user-specified data in user-specified chunks
//@ and verifies that incoming data matches specified samples.
//@
//@ If something is unexpected, Websocat will exit (panic).
//@
//@ Argument is a specially formatted string describing operations, separated by `|` character.
//@
//@ Operations:
//@
//@ * `R` - make the socket return specified chunk of data
//@ * `W` - make the socket wait for incoming data and check if it matches the sample
//@ * `ER` / `EW` - inject read or write error
//@ * `T0` ... `T9` - sleep for some time interval, from small to large.
//@ * `N` - set name of the mock object
//@ 
//@ See full description of operators on https://docs.rs/tokio-io-mock-fork/latest/tokio_io_mock_fork/struct.Builder.html#method.text_scenario
//@
//@ Example: `R hello|R world|W ping |R pong|T5|R zero byte \0 other escapes \| \xff \r\n\t|EW`
fn mock_stream_socket(ctx: NativeCallContext, content: String) -> RhResult<Handle<StreamSocket>> {
    let mut builder = tokio_io_mock_fork::Builder::new();

    if let Err(e) = builder.text_scenario(&content) {
        if let Some(c) = e.chararter {
            return Err(ctx.err(format!(
                "Invalid character `{}` at position {} in state `{:?}` when parsing content of a mock_stream_socket",
                std::ascii::escape_default(c), e.position, e.state,
            )));
        } else {
            return Err(ctx.err(format!(
                "EOF unexpected in state `{:?}` when parsing content of a mock_stream_socket",
                e.state,
            )));
        }
    }

    let io = builder.enable_shutdown_checking().build();

    let (r, w) = tokio::io::split(io);

    Ok(StreamSocket {
        read: Some(StreamRead {
            reader: Box::pin(r),
            prefix: Default::default(),
        }),
        write: Some(StreamWrite {
            writer: Box::pin(w),
        }),
        close: None,
        fd: None,
    }
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("mock_stream_socket", mock_stream_socket);
}
