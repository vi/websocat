use std::{net::SocketAddr, task::Poll};

use crate::scenario_executor::{
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils1::{SimpleErr, ToNeutralAddress},
};
use futures::FutureExt;
use rhai::{Dynamic, Engine, NativeCallContext};
use tokio::{io::ReadBuf, net::UdpSocket};
use tracing::{debug, debug_span, info, warn};

use crate::scenario_executor::types::Handle;
use std::sync::{Arc, RwLock};

use super::{
    types::{BufferFlag, PacketRead, PacketReadResult, PacketWrite},
    utils1::RhResult, utils2::{Defragmenter, DefragmenterAddChunkResult},
};

struct UdpAddrInner {
    target_address: SocketAddr,
    address_change_counter: u32,
}

struct UdpInner {
    s: UdpSocket,
    peer: RwLock<UdpAddrInner>,
}

struct UdpSend {
    s: Arc<UdpInner>,
    sendto_mode: bool,
    degragmenter: Defragmenter,
    inhibit_send_errors: bool,
}

fn new_udp_endpoint(
    s: UdpSocket,
    toaddr: SocketAddr,
    sendto_mode: bool,
    allow_other_addresses: bool,
    redirect_to_last_seen_address: bool,
    connect_to_first_seen_address: bool,
    tag_as_text: bool,
    inhibit_send_errors: bool,
) -> (UdpSend, UdpRecv) {
    let inner = Arc::new(UdpInner {
        s,
        peer: RwLock::new(UdpAddrInner {
            target_address: toaddr,
            address_change_counter: 0,
        }),
    });
    (
        UdpSend {
            s: inner.clone(),
            sendto_mode,
            degragmenter: Defragmenter::new(),
            inhibit_send_errors,
        },
        UdpRecv {
            s: inner,
            sendto_mode,
            allow_other_addresses,
            redirect_to_last_seen_address,
            connect_to_first_seen_address,
            tag_as_text,
        },
    )
}

impl PacketWrite for UdpSend {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: super::types::BufferFlags,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        
        let data : &[u8] = match this.degragmenter.add_chunk(buf, flags) {
            DefragmenterAddChunkResult::DontSendYet => {
                return Poll::Ready(Ok(()));
            }
            DefragmenterAddChunkResult::Continunous(x) => x,
        };

        let mut inhibit_send_errors = this.inhibit_send_errors;

        let ret = if !this.sendto_mode {
            this.s.s.poll_send(cx, data)
        } else {
            let addr = this.s.peer.read().unwrap().target_address;
            if addr.ip().is_unspecified() {
                inhibit_send_errors = true;
            }
            this.s.s.poll_send_to(cx, data, addr)
        };

        match ret {
            Poll::Ready(Ok(n)) => {
                if n != data.len() {
                    warn!("short UDP send");
                }
            }
            Poll::Ready(Err(e)) => {
                this.degragmenter.clear();
                if inhibit_send_errors {
                    warn!("Failed to send to UDP socket: {e}");
                } else {
                    return Poll::Ready(Err(e));
                }
            }
            Poll::Pending => return Poll::Pending,
        }

        this.degragmenter.clear();
        Poll::Ready(Ok(()))
    }
}

#[derive(Clone)]
struct UdpRecv {
    s: Arc<UdpInner>,
    sendto_mode: bool,
    allow_other_addresses: bool,
    redirect_to_last_seen_address: bool,
    connect_to_first_seen_address: bool,
    tag_as_text: bool,
}

impl PacketRead for UdpRecv {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<PacketReadResult>> {
        let this = self.get_mut();
        let flags = if this.tag_as_text {
            BufferFlag::Text.into()
        } else {
            Default::default()
        };
        if !this.sendto_mode {
            let mut rb = ReadBuf::new(buf);
            match this.s.s.poll_recv(cx, &mut rb) {
                Poll::Ready(Ok(())) => {
                    return Poll::Ready(Ok(PacketReadResult {
                        flags,
                        buffer_subset: 0..(rb.filled().len()),
                    }))
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        loop {
            let mut rb = ReadBuf::new(buf);
            let from: SocketAddr = match this.s.s.poll_recv_from(cx, &mut rb) {
                Poll::Ready(Ok(x)) => x,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };

            let savedaddr = this.s.peer.read().unwrap();
            if savedaddr.target_address != from {
                if !this.allow_other_addresses {
                    info!("Ignored incoming UDP datagram from a foreign address: {from}");
                    continue;
                }
                if this.redirect_to_last_seen_address {
                    drop(savedaddr);
                    let mut savedaddr = this.s.peer.write().unwrap();
                    savedaddr.target_address = from;
                    savedaddr.address_change_counter += 1;

                    info!(
                        "Updated UDP peer address to {from} (number of address changes: {}",
                        savedaddr.address_change_counter
                    );

                    if this.connect_to_first_seen_address {
                        match this.s.s.connect(from).now_or_never() {
                            Some(Ok(())) => {
                                this.sendto_mode = false;
                            }
                            Some(Err(e)) => return Poll::Ready(Err(e)),
                            None => panic!(
                                "UDP connect to specific address not completed immeidately somehow"
                            ),
                        }
                    }
                }
            }
            return Poll::Ready(Ok(PacketReadResult {
                flags,
                buffer_subset: 0..(rb.filled().len()),
            }));
        }
    }
}

//@ Create a single Datagram Socket that is bound to a UDP port,
//@ typically for connecting to a specific UDP endpoint
//@
//@ The node does not have it's own buffer size - the buffer is supplied externally
fn udp_socket(ctx: NativeCallContext, opts: Dynamic) -> RhResult<Handle<DatagramSocket>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "udp_socket");
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct UdpOpts {
        //@ Send datagrams to and expect datagrams from this address.
        addr: SocketAddr,

        //@ Specify address to bind the socket to.
        //@ By default it binds to `0.0.0.0:0` or `[::]:0`
        bind: Option<SocketAddr>,

        //@ Use `sendto` instead of `connect` + `send`.
        //@ This mode ignores ICMP reports that target is not reachable.
        #[serde(default)]
        sendto_mode: bool,

        //@ Do not filter out incoming datagrams from addresses other than `addr`.
        //@ Useless without `sendto_mode`.
        #[serde(default)]
        allow_other_addresses: bool,

        //@ Send datagrams to address of the last seen incoming datagrams,
        //@ using `addr` only as initial address until more data is received.
        //@ Useless without `allow_other_addresses`. May have security implications.
        #[serde(default)]
        redirect_to_last_seen_address: bool,

        //@ When using `redirect_to_last_seen_address`, lock the socket
        //@ to that address, preventing more changes and providing disconnects.
        //@ Useless without `redirect_to_last_seen_address`.
        #[serde(default)]
        connect_to_first_seen_address: bool,

        //@ Tag incoming UDP datagrams to be sent as text WebSocket messages
        //@ instead of binary.
        //@ Note that Websocat does not check for UTF-8 correctness and may
        //@ send non-compiant text WebSocket messages.
        #[serde(default)]
        tag_as_text: bool,

        //@ Do not exit if `sendto` returned an error.
        #[serde(default)]
        inhibit_send_errors: bool,
    }
    let opts: UdpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));
    debug!(parent: &span, addr=%opts.addr, "options parsed");

    let to_addr = opts.addr;
    let bind_addr = opts.bind.unwrap_or(to_addr.to_neutral_address());

    let Some(Ok(s)) = UdpSocket::bind(bind_addr).now_or_never() else {
        return Err(ctx.err("Failed to bind UDP socket"));
    };
    if !opts.sendto_mode {
        match s.connect(to_addr).now_or_never() {
            Some(Ok(())) => (),
            _ => return Err(ctx.err("Failed to connect UDP socket")),
        }
    }

    let (us, ur) = new_udp_endpoint(
        s,
        to_addr,
        opts.sendto_mode,
        opts.allow_other_addresses,
        opts.redirect_to_last_seen_address,
        opts.connect_to_first_seen_address,
        opts.tag_as_text,
        opts.inhibit_send_errors,
    );

    let s = DatagramSocket {
        read: Some(DatagramRead { src: Box::pin(ur) }),
        write: Some(DatagramWrite { snk: Box::pin(us) }),
        close: None,
    };
    debug!(s=?s, "created");
    Ok(s.wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("udp_socket", udp_socket);
}
