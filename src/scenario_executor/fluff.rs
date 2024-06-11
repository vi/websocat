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

struct TrivialPkts {
    n: u8,
}

impl PacketRead for TrivialPkts {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<BufferFlags>> {
        let mut this = self.as_mut();
        if this.n == 0 {
            return Poll::Ready(Ok(BufferFlag::Eof.into()));
        } else {
            buf.put_slice(format!("{}", this.n).as_bytes());
            this.n -= 1;
            return Poll::Ready(Ok(BufferFlag::Text.into()));
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
        buf: &mut tokio::io::ReadBuf<'_>,
        flags: BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        eprint!("P len={}", buf.filled().len());
        if flags.contains(BufferFlag::Text) {
            eprint!(" [T]");
        }
        if flags.contains(BufferFlag::Eof) {
            eprint!(" [E]");
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
