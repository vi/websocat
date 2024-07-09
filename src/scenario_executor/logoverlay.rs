use std::{ops::Deref, pin::Pin, task::Poll};

use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::{debug, debug_span};

use crate::scenario_executor::{
    types::{Handle, StreamRead},
    utils::{ExtractHandleOrFail, RhResult},
};

use super::{
    types::{
        DatagramRead, DatagramSocket, DatagramWrite, PacketRead, PacketReadResult,
        PacketWrite, StreamSocket, StreamWrite,
    },
    utils::{DisplayBufferFlags, HandleExt},
};

// Duplicated to aid auto-documenter script
#[derive(Clone)]
struct LoggerOptsShared {
    verbose: bool,
    prefix: String,
    omit_content: bool,
    hex: bool,
}

fn render_content(buf: &[u8], hex_mode: bool) -> String {
    if hex_mode {
        hex::encode(buf)
    } else {
        let mut s = String::with_capacity(buf.len() + 2);
        s.push('"');
        for x in buf.iter().cloned().map(std::ascii::escape_default) {
            s.push_str(String::from_utf8_lossy(&x.collect::<Vec<u8>>()).as_ref());
        }
        s.push('"');
        s
    }
}

struct StreamReadLogger {
    inner: StreamRead,
    opts: LoggerOptsShared,
}

impl AsyncRead for StreamReadLogger {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let from_prefix = !this.inner.prefix.is_empty();
        let log_prefix: &str = &this.opts.prefix;
        let maybebufcap_storage;
        let maybebufcap: &str = if this.opts.verbose {
            maybebufcap_storage = format!("bufcap={} ", buf.capacity());
            maybebufcap_storage.as_ref()
        } else {
            ""
        };
        let maybefromprefix = if from_prefix && this.opts.verbose {
            &"from_prefix "
        } else {
            ""
        };
        match AsyncRead::poll_read(Pin::new(&mut this.inner), cx, buf) {
            Poll::Ready(ret) => match ret {
                Ok(()) => {
                    if !this.opts.omit_content {
                        eprintln!(
                            "{log_prefix}{maybebufcap}{maybefromprefix}{} {}",
                            buf.filled().len(),
                            render_content(buf.filled(), this.opts.hex)
                        );
                    } else {
                        eprintln!(
                            "{log_prefix}{maybebufcap}{maybefromprefix}{}",
                            buf.filled().len()
                        );
                    }
                    Poll::Ready(Ok(()))
                }
                Err(e) => {
                    eprintln!("{log_prefix}{maybebufcap}error {e}");
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => {
                if this.opts.verbose {
                    eprintln!("{log_prefix}{maybebufcap}pending");
                }
                Poll::Pending
            }
        }
    }
}

struct StreamWriteLogger {
    inner: StreamWrite,
    opts: LoggerOptsShared,
}

impl AsyncWrite for StreamWriteLogger {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let maybebufcap_storage;
        let maybebufcap: &str = if this.opts.verbose {
            maybebufcap_storage = format!("bufcap={} ", buf.len());
            maybebufcap_storage.as_ref()
        } else {
            ""
        };
        let verbose = this.opts.verbose;

        match AsyncWrite::poll_write(Pin::new(&mut this.inner.writer), cx, buf) {
            Poll::Ready(Ok(nbytes)) => {
                if !this.opts.omit_content {
                    eprintln!(
                        "{log_prefix}{maybebufcap}{} {}",
                        nbytes,
                        render_content(&buf[..nbytes], this.opts.hex)
                    );
                } else {
                    eprintln!("{log_prefix}{maybebufcap}{}", nbytes,);
                }
                Poll::Ready(Ok(nbytes))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}{maybebufcap}error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}{maybebufcap}pending");
                }
                Poll::Pending
            }
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let verbose = this.opts.verbose;
        match AsyncWrite::poll_shutdown(Pin::new(&mut this.inner.writer), cx) {
            Poll::Ready(Ok(())) => {
                if verbose {
                    eprintln!("{log_prefix}flush");
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}flush error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}flush pending");
                }
                Poll::Pending
            }
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let verbose = this.opts.verbose;
        match AsyncWrite::poll_shutdown(Pin::new(&mut this.inner.writer), cx) {
            Poll::Ready(Ok(())) => {
                eprintln!("{log_prefix}shutdown");
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}shutdown error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}shutdown pending");
                }
                Poll::Pending
            }
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let maybebufcap_storage;
        let maybebufcap: &str = if this.opts.verbose {
            maybebufcap_storage = format!("slices={} ", bufs.len());
            maybebufcap_storage.as_ref()
        } else {
            ""
        };
        let verbose = this.opts.verbose;

        match AsyncWrite::poll_write_vectored(Pin::new(&mut this.inner.writer), cx, bufs) {
            Poll::Ready(Ok(nbytes)) => {
                if !this.opts.omit_content {
                    let mut content = Vec::with_capacity(nbytes);
                    let mut remaining = nbytes;
                    for b in bufs {
                        let buf: &[u8] = b.deref();
                        let maxbytes = remaining.min(buf.len());
                        let bb = &buf[..maxbytes];
                        content.extend_from_slice(bb);
                        remaining -= maxbytes;
                        if remaining == 0 {
                            break;
                        }
                    }
                    eprintln!(
                        "{log_prefix}{maybebufcap} {} {}",
                        nbytes,
                        render_content(&content, this.opts.hex)
                    );
                } else {
                    eprintln!("{log_prefix}{maybebufcap} {}", nbytes);
                }
                Poll::Ready(Ok(nbytes))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}{maybebufcap}error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}{maybebufcap}pending");
                }
                Poll::Pending
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.writer.is_write_vectored()
    }
}

//@ Wrap stream socket in an overlay that logs every inner read and write to stderr.
//@ Stderr is assumed to be always available. Backpressure would cause
//@ whole process to stop serving connections and inability to log
//@ may abort the process.
//@
//@ It is OK a if read or write handle of the source socket is null - resulting socket
//@ would also be incomplete. This allows to access the logger having only reader
//@ or writer instead of a complete socket.
//@
//@ This component is not performance-optimised and is intended for mostly for debugging.
fn stream_logger(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<StreamSocket>,
) -> RhResult<Handle<StreamSocket>> {
    let span = debug_span!("stream_logger");
    #[derive(serde::Deserialize)]
    struct LoggerOpts {
        //@ Show more messages and more info within messages
        #[serde(default)]
        verbose: bool,

        //@ Prepend this instead of "READ " to each line printed to stderr
        read_prefix: Option<String>,

        //@ Prepend this instead of "WRITE " to each line printed to stderr
        write_prefix: Option<String>,

        //@ Do not log full content of the stream, just the chunk lengths.
        #[serde(default)]
        omit_content: bool,

        //@ Use hex lines instead of string literals with espaces
        #[serde(default)]
        hex: bool,
    }
    let opts: LoggerOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let mut wrapped = inner;

    let read_prefix = opts.read_prefix.unwrap_or("READ ".to_owned());
    let write_prefix = opts.write_prefix.unwrap_or("WRITE ".to_owned());

    if let Some(r) = wrapped.read.take() {
        wrapped.read = Some(StreamRead {
            reader: (Box::pin(StreamReadLogger {
                inner: r,
                opts: LoggerOptsShared {
                    verbose: opts.verbose,
                    prefix: read_prefix,
                    omit_content: opts.omit_content,
                    hex: opts.hex,
                },
            })),
            prefix: Default::default(),
        });
    } else {
        if opts.verbose {
            eprintln!("{read_prefix}There is no read handle in this socket");
        }
    }

    if let Some(w) = wrapped.write.take() {
        wrapped.write = Some(StreamWrite {
            writer: (Box::pin(StreamWriteLogger {
                inner: w,
                opts: LoggerOptsShared {
                    verbose: opts.verbose,
                    prefix: write_prefix,
                    omit_content: opts.omit_content,
                    hex: opts.hex,
                },
            })),
        });
    } else {
        if opts.verbose {
            eprintln!("{write_prefix}There is no read handle in this socket");
        }
    }

    Ok(Some(wrapped).wrap())
}

struct DatagramReadLogger {
    inner: DatagramRead,
    opts: LoggerOptsShared,
}

impl PacketRead for DatagramReadLogger {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let maybebufcap_storage;
        let maybebufcap: &str = if this.opts.verbose {
            maybebufcap_storage = format!("bufcap={} ", buf.len());
            maybebufcap_storage.as_ref()
        } else {
            ""
        };
        let verbose = this.opts.verbose;
        match PacketRead::poll_read(this.inner.src.as_mut(), cx, buf) {
            Poll::Ready(Ok(x)) => {
                let maybe_flags_storge;
                let maybe_flags = if verbose {
                    maybe_flags_storge=format!(" [{}]", DisplayBufferFlags(x.flags));
                    &maybe_flags_storge
                } else {
                    ""
                };
                if !this.opts.omit_content {
                    eprintln!(
                        "{log_prefix}{maybebufcap}{} {}{maybe_flags}",
                        x.buffer_subset.len(),
                        render_content(&buf[x.buffer_subset.clone()], this.opts.hex)
                    );
                } else {
                    eprintln!("{log_prefix}{maybebufcap}{}{maybe_flags}", x.buffer_subset.len());
                }
                Poll::Ready(Ok(x))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}{maybebufcap}error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}{maybebufcap}pending");
                }
                Poll::Pending
            }
        }
    }
}

struct DatagramWriteLogger {
    inner: DatagramWrite,
    opts: LoggerOptsShared,
    already_logged_this_write: bool,
}

impl PacketWrite for DatagramWriteLogger {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: super::types::BufferFlags,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let log_prefix: &str = &this.opts.prefix;
        let maybebufcap_storage;
        let maybebufcap: &str = if this.opts.verbose {
            maybebufcap_storage = format!("bufcap={} ", buf.len());
            maybebufcap_storage.as_ref()
        } else {
            ""
        };
        let verbose = this.opts.verbose;

        if !this.already_logged_this_write {
            let maybe_flags_storge;
            let maybe_flags = if verbose {
                maybe_flags_storge=format!(" [{}]", DisplayBufferFlags(flags));
                &maybe_flags_storge
            } else {
                ""
            };
            if !this.opts.omit_content {
                eprintln!(
                    "{log_prefix}{maybebufcap}{} {}{maybe_flags}",
                    buf.len(),
                    render_content(buf, this.opts.hex)
                );
            } else {
                eprintln!("{log_prefix}{maybebufcap}{}{maybe_flags}", buf.len());
            }
            this.already_logged_this_write = true;
        }

        match PacketWrite::poll_write(this.inner.snk.as_mut(), cx, buf, flags) {
            Poll::Ready(Ok(())) => {
                this.already_logged_this_write=false;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                eprintln!("{log_prefix}error {e}");
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if verbose {
                    eprintln!("{log_prefix}pending");
                }
                Poll::Pending
            }
        }
    }
}

//@ Wrap datagram socket in an overlay that logs every inner read and write to stderr.
//@ Stderr is assumed to be always available. Backpressure would cause
//@ whole process to stop serving connections and inability to log
//@ may abort the process.
//@
//@ It is OK if a read or write handle of the source socket is null - resulting socket
//@ would also be incomplete. This allows to access the logger having only reader
//@ or writer instead of a complete socket.
//@
//@ This component is not performance-optimised and is intended for mostly for debugging.
fn datagram_logger(
    ctx: NativeCallContext,
    opts: Dynamic,
    inner: Handle<DatagramSocket>,
) -> RhResult<Handle<DatagramSocket>> {
    let span = debug_span!("datagram_logger");
    #[derive(serde::Deserialize)]
    struct LoggerOpts {
        //@ Show more messages and more info within messages
        #[serde(default)]
        verbose: bool,

        //@ Prepend this instead of "READ " to each line printed to stderr
        read_prefix: Option<String>,

        //@ Prepend this instead of "WRITE " to each line printed to stderr
        write_prefix: Option<String>,

        //@ Do not log full content of the stream, just the chunk lengths.
        #[serde(default)]
        omit_content: bool,

        //@ Use hex lines instead of string literals with espaces
        #[serde(default)]
        hex: bool,
    }
    let opts: LoggerOpts = rhai::serde::from_dynamic(&opts)?;
    let inner = ctx.lutbar(inner)?;
    debug!(parent: &span, inner=?inner, "options parsed");
    let mut wrapped = inner;

    let read_prefix = opts.read_prefix.unwrap_or("READ ".to_owned());
    let write_prefix = opts.write_prefix.unwrap_or("WRITE ".to_owned());

    if let Some(r) = wrapped.read.take() {
        wrapped.read = Some(DatagramRead {
            src: (Box::pin(DatagramReadLogger {
                inner: r,
                opts: LoggerOptsShared {
                    verbose: opts.verbose,
                    prefix: read_prefix,
                    omit_content: opts.omit_content,
                    hex: opts.hex,
                },
            })),
        });
    } else {
        if opts.verbose {
            eprintln!("{read_prefix}There is no read handle in this socket");
        }
    }

    if let Some(w) = wrapped.write.take() {
        wrapped.write = Some(DatagramWrite {
            snk: (Box::pin(DatagramWriteLogger {
                inner: w,
                opts: LoggerOptsShared {
                    verbose: opts.verbose,
                    prefix: write_prefix,
                    omit_content: opts.omit_content,
                    hex: opts.hex,
                },
                already_logged_this_write: false,
            })),
        });
    } else {
        if opts.verbose {
            eprintln!("{write_prefix}There is no read handle in this socket");
        }
    }

    Ok(Some(wrapped).wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("stream_logger", stream_logger);
    engine.register_fn("datagram_logger", datagram_logger);
}
