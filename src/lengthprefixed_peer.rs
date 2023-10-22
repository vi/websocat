use futures::future::ok;

use std::rc::Rc;

use crate::{io_other_error, simple_err, peer_strerr};

use super::{BoxedNewPeerFuture, Peer};
use super::{ConstructParams, PeerConstructor, Specifier};

use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::io::Error as IoError;

#[derive(Debug)]
pub struct LengthPrefixed<T: Specifier>(pub T);
impl<T: Specifier> Specifier for LengthPrefixed<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, _| {
            lengthprefixed_peer(
                p,
                cp.program_options.lengthprefixed_header_bytes,
                cp.program_options.lengthprefixed_little_endian,
            )
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = LengthPrefixedClass,
    target = LengthPrefixed,
    prefixes = ["lengthprefixed:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
Turn stream of bytes to/from data packets with length-prefixed framing.  [A]

You can choose the number of header bytes (1 to 8) and endianness. Default is 4 bytes big endian.

This affects both reading and writing - attach this overlay to stream specifier to turn it into a packet-orineted specifier.

Mind the buffer size (-B). All packets should fit in there.

Examples:

    websocat -u -b udp-l:127.0.0.1:1234 lengthprefixed:writefile:test.dat

    websocat -u -b lengthprefixed:readfile:test.dat udp:127.0.0.1:1235

This would save incoming UDP packets to a file, then replay the datagrams back to UDP socket

    websocat -b lengthprefixed:- ws://127.0.0.1:1234/ --binary-prefix=B --text-prefix=T

This allows to mix and match text and binary WebSocket messages to and from stdio without the base64 overhead.
"#
);

pub fn lengthprefixed_peer(
    inner_peer: Peer,
    num_bytes_in_length_prefix: usize,
    little_endian: bool,
) -> BoxedNewPeerFuture {
    if num_bytes_in_length_prefix < 1 || num_bytes_in_length_prefix > 8 {
        return peer_strerr("Number of header bytes for lengthprefixed overlay should be from 1 to 8");
    }

    let (length_starting_pos, length_ending_pos) = if little_endian {
        (0, num_bytes_in_length_prefix)
    } else {
        (8 - num_bytes_in_length_prefix, 8)
    };
    let reader = Lengthprefixed2PacketWrapper {
        inner: inner_peer.0,
        length_buffer: [0; 8],
        length_starting_pos,
        length_pos: length_starting_pos,
        length_ending_pos: length_ending_pos,
        little_endian,
        data_read_so_far: 0,
    };
    let writer = Packet2LengthPrefixedWrapper {
        inner: inner_peer.1,
        length_buffer: [0; 8],
        length_starting_pos,
        length_pos: length_starting_pos,
        length_ending_pos: length_ending_pos,
        little_endian,
        data_written_so_far: 0,
    };
    let thepeer = Peer::new(reader, writer, inner_peer.2);
    Box::new(ok(thepeer)) as BoxedNewPeerFuture
}
struct Lengthprefixed2PacketWrapper {
    inner: Box<dyn AsyncRead>,
    length_buffer: [u8; 8],
    length_starting_pos: usize,
    length_ending_pos: usize,
    length_pos: usize,
    little_endian: bool,
    data_read_so_far: usize,
}
impl Read for Lengthprefixed2PacketWrapper {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        loop {
            assert!(self.length_pos <= self.length_ending_pos);
            assert!(self.length_pos >= self.length_starting_pos);
            if self.length_ending_pos != self.length_pos {
                match self
                    .inner
                    .read(&mut self.length_buffer[self.length_pos..self.length_ending_pos])
                {
                    Err(e) => return Err(e),
                    Ok(0) => {
                        if self.length_pos != self.length_starting_pos {
                            error!("Possibly trimmed length-prefixed data.")
                        }
                        return Ok(0);
                    }
                    Ok(n) => {
                        self.length_pos += n;
                        continue;
                    }
                }
            } else {
                let packet_len = if self.little_endian {
                    u64::from_le_bytes(self.length_buffer)
                } else {
                    u64::from_be_bytes(self.length_buffer)
                };
                if packet_len >= (buf.len() as u64) {
                    error!("Failed to process too big packet. You may need to increase the -B buffer size.");
                    return Err(io_other_error(simple_err("Packet length overflow".into())));
                }
                let packet_len = packet_len as usize;
                if packet_len == 0 {
                    return Ok(0);
                }

                if self.data_read_so_far == packet_len {
                    self.data_read_so_far = 0;
                    self.length_buffer = [0; 8];
                    self.length_pos = self.length_starting_pos;
                    return Ok(packet_len);
                }

                // Assume we are called with the same buffer until we return success, so we
                // can use buffer as a persistent scratch space
                match self.inner.read(&mut buf[self.data_read_so_far..packet_len]) {
                    Err(e) => return Err(e),
                    Ok(0) => {
                        return Err(io_other_error(simple_err("Data trimmed".into())));
                    }
                    Ok(n) => {
                        self.data_read_so_far += n;
                        continue;
                    }
                }
            }
        }
    }
}
impl AsyncRead for Lengthprefixed2PacketWrapper {}

struct Packet2LengthPrefixedWrapper {
    inner: Box<dyn AsyncWrite>,
    length_buffer: [u8; 8],
    length_starting_pos: usize,
    length_ending_pos: usize,
    length_pos: usize,
    little_endian: bool,
    data_written_so_far: usize,
}

impl Write for Packet2LengthPrefixedWrapper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Assuming `write` is retried with the same buffer when we return WouldBlock
        loop {
            if self.length_pos == self.length_starting_pos {
                if self.little_endian {
                    self.length_buffer = (buf.len() as u64).to_le_bytes()
                } else {
                    self.length_buffer = (buf.len() as u64).to_be_bytes()
                }
            }
            if self.length_pos < self.length_ending_pos {
                match self
                    .inner
                    .write(&self.length_buffer[self.length_pos..self.length_ending_pos])
                {
                    Err(x) => return Err(x),
                    Ok(n) => self.length_pos += n,
                }
                continue;
            }

            if self.data_written_so_far == buf.len() {
                self.data_written_so_far = 0;
                self.length_pos = self.length_starting_pos;
                self.length_buffer = [0; 8];
                return Ok(buf.len());
            }

            match self.inner.write(&buf[self.data_written_so_far..]) {
                Err(e) => return Err(e),
                Ok(n) => self.data_written_so_far += n,
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl AsyncWrite for Packet2LengthPrefixedWrapper {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.inner.shutdown()
    }
}
