use futures;
use futures::Async;
use futures::Future;
use std;
use std::io;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_io::{AsyncRead, AsyncWrite};

use std::fs::{File, OpenOptions};
use std::rc::Rc;

use super::{BoxedNewPeerFuture, Peer, Result};

use super::{once, ConstructParams, PeerConstructor, Specifier};

#[derive(Clone, Debug)]
pub struct ReadFile(pub PathBuf);
impl Specifier for ReadFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = File::open(p)?;
            Ok(Peer::new(ReadFileWrapper(f), super::trivial_peer::DevNull, None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = ReadFileClass,
    target = ReadFile,
    prefixes = ["readfile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Synchronously read a file. Argument is a file path.

Blocking on operations with the file pauses the whole process

Example: Serve the file once per connection, ignore all replies.

    websocat ws-l:127.0.0.1:8000 readfile:hello.json

"#
);

#[derive(Clone, Debug)]
pub struct WriteFile(pub PathBuf);
impl Specifier for WriteFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = File::create(p)?;
            Ok(Peer::new(super::trivial_peer::DevNull, WriteFileWrapper(f), None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = WriteFileClass,
    target = WriteFile,
    prefixes = ["writefile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"

Synchronously truncate and write a file.

Blocking on operations with the file pauses the whole process

Example:

    websocat ws-l:127.0.0.1:8000 writefile:data.txt

"#
);

#[derive(Clone, Debug)]
pub struct AppendFile(pub PathBuf);
impl Specifier for AppendFile {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = OpenOptions::new().create(true).append(true).open(p)?;
            Ok(Peer::new(super::trivial_peer::DevNull, WriteFileWrapper(f), None))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = AppendFileClass,
    target = AppendFile,
    prefixes = ["appendfile:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"

Synchronously append a file.

Blocking on operations with the file pauses the whole process

Example: Logging all incoming data from WebSocket clients to one file

    websocat -u ws-l:127.0.0.1:8000 reuse:appendfile:log.txt
"#
);

// Timestamped file format constants.
// Header layout (all big-endian, 16 bytes total):
//   [magic u32 = 0xC0DEBABE][timestamp_us u64][length u32]
const TS_MAGIC: u32 = 0xC0DE_BABE;
const TS_HEADER_LEN: usize = 16;

fn write_ts_header(f: &mut File, data_len: usize) -> IoResult<()> {
    let ts_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let mut hdr = [0u8; TS_HEADER_LEN];
    hdr[0..4].copy_from_slice(&TS_MAGIC.to_be_bytes());
    hdr[4..12].copy_from_slice(&ts_us.to_be_bytes());
    hdr[12..16].copy_from_slice(&(data_len as u32).to_be_bytes());
    f.write_all(&hdr)
}

fn parse_ts_file(path: &Path) -> IoResult<Vec<(u64, Vec<u8>)>> {
    let mut f = File::open(path)?;
    let mut chunks = Vec::new();
    loop {
        let mut hdr = [0u8; TS_HEADER_LEN];
        match f.read_exact(&mut hdr) {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let magic = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        if magic != TS_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "readfile_ts: invalid magic cookie (not a writefile_ts: capture?)",
            ));
        }
        let ts = u64::from_be_bytes([
            hdr[4], hdr[5], hdr[6], hdr[7], hdr[8], hdr[9], hdr[10], hdr[11],
        ]);
        let len = u32::from_be_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
        let mut data = vec![0u8; len];
        f.read_exact(&mut data)?;
        chunks.push((ts, data));
    }
    Ok(chunks)
}

#[derive(Clone, Debug)]
pub struct WriteFileTs(pub PathBuf);
impl Specifier for WriteFileTs {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let f = File::create(p)?;
            Ok(Peer::new(
                super::trivial_peer::DevNull,
                WriteFileTsWrapper(f),
                None,
            ))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = WriteFileTsClass,
    target = WriteFileTs,
    prefixes = ["writefile_ts:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Like writefile:, but prepends each chunk with a 16-byte timestamped header
so the file can be replayed with accurate timing using readfile_ts:.

Header layout (all big-endian):
  [magic u32 = 0xC0DEBABE][timestamp_us u64][length u32]

Timestamp is microseconds since Unix epoch.

Example: capture a WebSocket stream with timing

    websocat -u -n ws://example.com/sock writefile_ts:capture.bin

"#
);

#[derive(Clone, Debug)]
pub struct ReadFileTs(pub PathBuf);
impl Specifier for ReadFileTs {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let chunks = parse_ts_file(p)?;
            let start_ts = chunks.first().map(|(ts, _)| *ts).unwrap_or(0);
            Ok(Peer::new(
                ReadFileTsWrapper {
                    chunks,
                    idx: 0,
                    pos: 0,
                    start_instant: Instant::now(),
                    start_ts,
                    delay: None,
                    loop_playback: false,
                },
                super::trivial_peer::DevNull,
                None,
            ))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = ReadFileTsClass,
    target = ReadFileTs,
    prefixes = ["readfile_ts:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Read a file previously written by writefile_ts: and replay its chunks with
the original inter-chunk timing preserved. Headers are stripped; only raw
data is sent.

Use --binary to prevent auto line-mode insertion, which would otherwise
buffer all chunks until EOF waiting for a newline that never arrives.

Example: replay a captured WebSocket stream

    websocat --binary -u readfile_ts:capture.bin ws://example.com/sock

Example: serve a recording to connecting clients

    websocat --binary ws-l:127.0.0.1:8000 readfile_ts:capture.bin

"#
);

#[derive(Clone, Debug)]
pub struct ReadFileTsLoop(pub PathBuf);
impl Specifier for ReadFileTsLoop {
    fn construct(&self, _: ConstructParams) -> PeerConstructor {
        fn gp(p: &Path) -> Result<Peer> {
            let chunks = parse_ts_file(p)?;
            let start_ts = chunks.first().map(|(ts, _)| *ts).unwrap_or(0);
            Ok(Peer::new(
                ReadFileTsWrapper {
                    chunks,
                    idx: 0,
                    pos: 0,
                    start_instant: Instant::now(),
                    start_ts,
                    delay: None,
                    loop_playback: true,
                },
                super::trivial_peer::DevNull,
                None,
            ))
        }
        once(Box::new(futures::future::result(gp(&self.0))) as BoxedNewPeerFuture)
    }
    specifier_boilerplate!(noglobalstate singleconnect no_subspec);
}
specifier_class!(
    name = ReadFileTsLoopClass,
    target = ReadFileTsLoop,
    prefixes = ["readfile_ts_loop:"],
    arg_handling = into,
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
Like readfile_ts:, but loops back to the first chunk after the last chunk
is sent, replaying indefinitely with the original inter-chunk timing.

Use --binary to prevent auto line-mode insertion.

Example: serve a looping recording to connecting clients

    websocat --binary ws-l:127.0.0.1:8000 readfile_ts_loop:capture.bin

"#
);

pub struct ReadFileWrapper(pub File);

impl AsyncRead for ReadFileWrapper {}
impl Read for ReadFileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.0.read(buf)
    }
}

struct WriteFileWrapper(File);

impl AsyncWrite for WriteFileWrapper {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(Async::Ready(()))
    }
}
impl Write for WriteFileWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}

struct WriteFileTsWrapper(File);

impl AsyncWrite for WriteFileTsWrapper {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        Ok(Async::Ready(()))
    }
}
impl Write for WriteFileTsWrapper {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        write_ts_header(&mut self.0, buf.len())?;
        self.0.write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}

struct ReadFileTsWrapper {
    chunks: Vec<(u64, Vec<u8>)>,
    idx: usize,
    pos: usize,
    start_instant: Instant,
    start_ts: u64,
    delay: Option<::tokio_timer::Delay>,
    loop_playback: bool,
}

impl AsyncRead for ReadFileTsWrapper {}
impl Read for ReadFileTsWrapper {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        loop {
            // Poll any active inter-chunk delay before doing anything else.
            if let Some(ref mut delay) = self.delay {
                match delay.poll() {
                    Ok(Async::NotReady) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "readfile_ts: waiting for replay delay",
                        ));
                    }
                    _ => {}
                }
                self.delay = None;
            }

            if self.idx >= self.chunks.len() {
                return Ok(0); // EOF
            }

            // Serve bytes from the current chunk.
            let chunk_len = self.chunks[self.idx].1.len();
            if self.pos < chunk_len {
                let n = buf.len().min(chunk_len - self.pos);
                let start = self.pos;
                buf[..n].copy_from_slice(&self.chunks[self.idx].1[start..start + n]);
                self.pos += n;
                return Ok(n);
            }

            // Current chunk exhausted; advance to the next one.
            self.idx += 1;
            self.pos = 0;

            if self.idx >= self.chunks.len() {
                if self.loop_playback && !self.chunks.is_empty() {
                    self.idx = 0;
                    self.start_instant = Instant::now();
                    continue;
                }
                return Ok(0); // EOF
            }

            // Schedule a delay so the next chunk fires at its original offset
            // relative to the first chunk's timestamp.
            let ts = self.chunks[self.idx].0;
            let offset_us = ts.saturating_sub(self.start_ts);
            let target = self.start_instant + Duration::from_micros(offset_us);
            if target > Instant::now() {
                self.delay = Some(::tokio_timer::Delay::new(target));
                // Loop back: the delay poll above will return NotReady and park the task.
            }
        }
    }
}
