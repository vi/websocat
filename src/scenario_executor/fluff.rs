use rhai::Engine;
use std::{
    sync::{Arc, Mutex},
    task::Poll,
};

use crate::scenario_executor::{
    types::{
        BufferFlag, BufferFlags, DatagramRead, DatagramWrite, Handle, PacketRead, PacketWrite,
    },
    utils::HandleExt,
};

use super::types::PacketReadResult;

struct TrivialPkts {
    n: u8,
}

impl PacketRead for TrivialPkts {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let mut this = self.as_mut();
        if this.n == 0 {
            return Poll::Ready(Ok(PacketReadResult {
                flags: BufferFlag::Eof.into(),
                buffer_subset: 0..0,
            }));
        } else {
            let msg = format!("{}", this.n);
            let msg =  msg.as_bytes();
            let l = msg.len();
            buf[..l].copy_from_slice(msg);
            this.n -= 1;
            return Poll::Ready(Ok(PacketReadResult {
                flags: BufferFlag::Text.into(),
                buffer_subset: 0..l,
            }));
        }
    }
}

fn trivial_pkts() -> Handle<DatagramRead> {
    Some(DatagramRead {
        src: Box::pin(TrivialPkts { n: 3 }),
    })
    .wrap()
}

struct DisplayPkts {}

impl PacketWrite for DisplayPkts {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        eprint!("P len={}", buf.len());
        if flags.contains(BufferFlag::Text) {
            eprint!(" [T]");
        }
        if flags.contains(BufferFlag::Eof) {
            eprint!(" [E]");
        }
        if flags.contains(BufferFlag::Ping) {
            eprint!(" [Pi]");
        }
        if flags.contains(BufferFlag::Pong) {
            eprint!(" [Po]");
        }
        if flags.contains(BufferFlag::NonFinalChunk) {
            eprint!(" [C]");
        }
        eprintln!();
        Poll::Ready(Ok(()))
    }
}

fn display_pkts() -> Handle<DatagramWrite> {
    let snk = Box::pin(DisplayPkts {});
    Arc::new(Mutex::new(Some(DatagramWrite { snk })))
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("trivial_pkts", trivial_pkts);
    engine.register_fn("display_pkts", display_pkts);
}
