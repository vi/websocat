use std::time::Duration;

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
//@ * 'ER' / `EW` - inject read or write error
//@ * 'T0` ... `T9` - sleep for some time interval, from small to large.
//@
//@ Example: `R hello|R world|W ping |R pong|T5|R zero byte \0 other escapes \| \xff \r\n\t|EW`
fn mock_stream_socket(ctx: NativeCallContext, content: String) -> RhResult<Handle<StreamSocket>> {
    let mut builder = tokio_test::io::Builder::new();

    #[derive(Copy, Clone)]
    enum BufferMode {
        Read,
        Write,
    }

    #[derive(Copy, Clone, Debug)]
    enum ParserState {
        WaitingForCommandCharacter,
        JustAfterCommandCharacter,
        InjectError,
        Wait,
        Normal,
        Escape,
        HexEscape1,
        HexEscape2(u8),
    }

    let mut buf = vec![];
    let mut bufmode = BufferMode::Read;
    let mut state = ParserState::WaitingForCommandCharacter;

    macro_rules! commit_buffer {
        () => {
            match bufmode {
                BufferMode::Write => {
                    builder.write(&buf);
                }
                BufferMode::Read => {
                    builder.read(&buf);
                }
            }
            buf.clear();
        };
    }

    use ParserState::*;
    for b in content.bytes() {
        match (state, b) {
            (WaitingForCommandCharacter | JustAfterCommandCharacter | InjectError | Wait, b' ') => {
            }
            (WaitingForCommandCharacter, b'R' | b'r') => {
                buf.clear();
                bufmode = BufferMode::Read;
                state = JustAfterCommandCharacter;
            }
            (WaitingForCommandCharacter, b'W' | b'w') => {
                buf.clear();
                bufmode = BufferMode::Write;
                state = JustAfterCommandCharacter;
            }
            (WaitingForCommandCharacter, b'|') => {}
            (WaitingForCommandCharacter, b'E') => {
                state = InjectError;
            }
            (WaitingForCommandCharacter, b'T') => {
                state = Wait;
            }
            (JustAfterCommandCharacter | Normal, b'|') => {
                commit_buffer!();
                state = WaitingForCommandCharacter;
            }
            (InjectError, b'R' | b'r') => {
                builder.read_error(std::io::ErrorKind::Other.into());
            }
            (InjectError, b'W' | b'w') => {
                builder.write_error(std::io::ErrorKind::Other.into());
            }
            (InjectError, b'|') => {
                state = WaitingForCommandCharacter;
            }
            (JustAfterCommandCharacter | Normal, b'\\') => {
                state = Escape;
            }
            (JustAfterCommandCharacter | Normal, b) => {
                buf.push(b);
                state = Normal;
            }
            (Escape, b'n') => {
                buf.push(b'\n');
                state = Normal;
            }
            (Escape, b'r') => {
                buf.push(b'\r');
                state = Normal;
            }
            (Escape, b'0') => {
                buf.push(b'\0');
                state = Normal;
            }
            (Escape, b't') => {
                buf.push(b'\t');
                state = Normal;
            }
            (Escape, b'x') => {
                state = HexEscape1;
            }
            (HexEscape1, x @ (b'0'..=b'9' | b'A'..=b'F' | b'a'..=b'f')) => {
                state = HexEscape2(x);
            }
            (HexEscape2(c1), c2 @ (b'0'..=b'9' | b'A'..=b'F' | b'a'..=b'f')) => {
                let mut b = [0];
                let s = [c1, c2];
                hex::decode_to_slice(s, &mut b).unwrap();
                buf.push(b[0]);
                state = Normal;
            }
            (Escape, b) => {
                buf.push(b);
                state = Normal;
            }
            (Wait, b @ (b'0'..=b'9')) => {
                let d = match b {
                    b'0' => Duration::from_millis(1),
                    b'1' => Duration::from_millis(3),
                    b'2' => Duration::from_millis(10),
                    b'3' => Duration::from_millis(33),
                    b'4' => Duration::from_millis(100),
                    b'5' => Duration::from_millis(333),
                    b'6' => Duration::from_secs(1),
                    b'7' => Duration::from_secs(10),
                    b'8' => Duration::from_secs(60),
                    b'9' => Duration::from_secs(3600),
                    _ => unreachable!(),
                };
                builder.wait(d);
                state = WaitingForCommandCharacter;
            }
            (s, b) => {
                return Err(ctx.err(format!(
                    "Invalid character `{}` in state {s:?} when parsing content of a mock_stream_socket",
                    std::ascii::escape_default(b),
                )));
            }
        }
    }

    match state {
        WaitingForCommandCharacter | InjectError => {}
        JustAfterCommandCharacter | Normal => {
            commit_buffer!();
        }
        s => {
            return Err(ctx.err(format!(
                "Invalid final state {s:?} when parsing content of a mock_stream_socket",
            )));
        }
    }

    let io = builder.build();

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
