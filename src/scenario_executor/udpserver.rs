use std::{net::SocketAddr, sync::Mutex, task::Poll, time::Duration};

use crate::scenario_executor::{
    scenario::{callback_and_continue, ScenarioAccess},
    types::{DatagramRead, DatagramSocket, DatagramWrite},
    utils1::{HandleExt, SimpleErr}, utils2::DefragmenterAddChunkResult,
};
use bytes::BytesMut;
use futures::{future::OptionFuture, FutureExt};
use lru::LruCache;
use rhai::{Dynamic, Engine, FnPtr, NativeCallContext};
use tokio::{net::UdpSocket, sync::mpsc::error::TrySendError, time::Instant};
use tracing::{debug, debug_span, error, trace, warn, Instrument};

use crate::scenario_executor::types::Handle;
use std::sync::Arc;

use super::{
    types::{BufferFlag, PacketRead, PacketReadResult, PacketWrite, Task},
    utils1::RhResult, utils2::Defragmenter,
};
use crate::scenario_executor::utils1::TaskHandleExt2;

struct VolatileClientInfo {
    deadline: Option<Instant>,
    removal_notifier: Option<tokio::sync::oneshot::Sender<()>>,
    sink: tokio::sync::mpsc::Sender<bytes::Bytes>,
}

impl VolatileClientInfo {
    fn dead(&self) -> bool {
        self.removal_notifier.is_none()
    }

    fn terminate(&mut self) {
        if let Some(rn) = self.removal_notifier.take() {
            let _ = rn.send(());
        }
    }
}

struct ClientInfo {
    addr: SocketAddr,
    v: Mutex<VolatileClientInfo>,
}

async fn hangup_monitor(
    ci: Arc<ClientInfo>,
    mut removal_notifier: tokio::sync::oneshot::Receiver<()>,
) {
    debug!(addr=?ci.addr, "Started hangup monitor");
    loop {
        trace!("hgmon loop");
        let (timeout, has_timeout): (OptionFuture<_>, bool) = {
            let mut l = ci.v.lock().unwrap();
            if l.dead() {
                trace!("hgmon dead");
                return;
            }
            let deadline = l.deadline;
            let now = Instant::now();
            if let Some(ref deadl) = deadline {
                if now >= *deadl {
                    debug!("Hangup monitor expired based on timeout");
                    l.terminate();
                    return;
                }
            }
            drop(l);
            (
                deadline.map(|d| tokio::time::sleep_until(d)).into(),
                deadline.is_some(),
            )
        };

        let do_expire = tokio::select! {
            biased;
            _ret = &mut removal_notifier => {
                true
            }
            _ret = timeout, if has_timeout => {
                // we loop around and check if possible updated dateline is really passed
                false
            }
        };

        if do_expire {
            debug!("Hangup monitor expired based on removal notifier");
            return;
        }
    }
}

struct UdpSend {
    s: Arc<UdpSocket>,
    ci: Arc<ClientInfo>,
    defragmenter: Defragmenter,
    inhibit_send_errors: bool,
}

impl UdpSend {
    fn new(s: Arc<UdpSocket>, ci: Arc<ClientInfo>, inhibit_send_errors: bool, max_send_datagram_size: usize) -> Self {
        Self {
            s,
            ci,
            defragmenter: Defragmenter::new(max_send_datagram_size),
            inhibit_send_errors,
        }
    }
}

impl PacketWrite for UdpSend {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
        flags: super::types::BufferFlags,
    ) -> std::task::Poll<std::io::Result<()>> {
        trace!("poll_write");
        let this = self.get_mut();


        let data : &[u8] = match this.defragmenter.add_chunk(buf, flags) {
            DefragmenterAddChunkResult::DontSendYet => {
                return Poll::Ready(Ok(()));
            }
            DefragmenterAddChunkResult::Continunous(x) => x,
            DefragmenterAddChunkResult::SizeLimitExceeded(_x) => {
                warn!("Exceeded maximum allowed outgoing datagram size. Closing this session.");
                return Poll::Ready(Err(std::io::ErrorKind::InvalidData.into()));
            }
        };

        let inhibit_send_errors = this.inhibit_send_errors;

        let addr = this.ci.addr;

        {
            let v = this.ci.v.lock().unwrap();
            if v.dead() {
                return Poll::Ready(Err(std::io::ErrorKind::ConnectionAborted.into()));
            }
        }

        let ret = this.s.poll_send_to(cx, data, addr);

        match ret {
            Poll::Ready(Ok(n)) => {
                if n != data.len() {
                    warn!("short UDP send");
                }
            }
            Poll::Ready(Err(e)) => {
                this.defragmenter.clear();
                if inhibit_send_errors {
                    warn!("Failed to send to UDP socket: {e}");
                } else {
                    return Poll::Ready(Err(e));
                }
            }
            Poll::Pending => return Poll::Pending,
        }

        this.defragmenter.clear();
        Poll::Ready(Ok(()))
    }
}

struct UdpRecv {
    recv: tokio::sync::mpsc::Receiver<bytes::Bytes>,
    tag_as_text: bool,
}

impl PacketRead for UdpRecv {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<PacketReadResult>> {
        trace!("poll_read");
        let this = self.get_mut();
        let flags = if this.tag_as_text {
            BufferFlag::Text.into()
        } else {
            Default::default()
        };

        let l;
        match this.recv.poll_recv(cx) {
            Poll::Ready(Some(b)) => {
                trace!(len = b.len(), "recv");
                if b.len() > buf.len() {
                    warn!("Incoming UDP datagram too big for a supplied buffer");
                    return Poll::Ready(Err(std::io::ErrorKind::InvalidInput.into()));
                }
                l = b.len();
                buf[..l].copy_from_slice(&b);
            }
            Poll::Ready(None) => {
                debug!("conn abort");
                return Poll::Ready(Err(std::io::ErrorKind::ConnectionAborted.into()));
            }
            Poll::Pending => return Poll::Pending,
        }

        Poll::Ready(Ok(PacketReadResult {
            flags,
            buffer_subset: 0..l,
        }))
    }
}


const fn default_max_send_datagram_size() -> usize { 4096 }

//@ Create a single Datagram Socket that is bound to a UDP port,
//@ typically for connecting to a specific UDP endpoint
fn udp_server(
    ctx: NativeCallContext,
    opts: Dynamic,
    continuation: FnPtr,
) -> RhResult<Handle<Task>> {
    let original_span = tracing::Span::current();
    let span = debug_span!(parent: original_span, "udp_server");
    let the_scenario = ctx.get_scenario()?;
    debug!(parent: &span, "node created");
    #[derive(serde::Deserialize)]
    struct UdpOpts {
        //@ Specify address to bind the socket to.
        bind: SocketAddr,

        //@ Mark the conection as closed when this number
        //@ of milliseconds elapse without a new datagram
        //@ from associated peer address
        timeout_ms: Option<u64>,

        //@ Maximum number of simultaneously connected clients.
        //@ If exceed, stale clients (based on the last received datagram) will be hung up.
        max_clients: Option<usize>,

        //@ Buffer size for receiving UDP datagrams.
        //@ Default is 4096 bytes.
        buffer_size: Option<usize>,

        //@ Queue length for distributing received UDP datagrams among spawned DatagramSocekts
        //@ Defaults to 1.
        qlen: Option<usize>,

        //@ Tag incoming UDP datagrams to be sent as text WebSocket messages
        //@ instead of binary.
        //@ Note that Websocat does not check for UTF-8 correctness and may
        //@ send non-compiant text WebSocket messages.
        #[serde(default)]
        tag_as_text: bool,

        //@ In case of one slow client handler, delay incoming UDP datagrams
        //@ instead of dropping them
        #[serde(default)]
        backpressure: bool,

        //@ Do not exit if `sendto` returned an error.
        #[serde(default)]
        inhibit_send_errors: bool,

        //@ Default defragmenter buffer limit
        #[serde(default="default_max_send_datagram_size")]
        max_send_datagram_size: usize,
    }
    let opts: UdpOpts = rhai::serde::from_dynamic(&opts)?;
    //span.record("addr", field::display(opts.addr));

    let bind_addr = opts.bind;

    let mut lru: LruCache<SocketAddr, Arc<ClientInfo>> = match opts.max_clients {
        None => LruCache::unbounded(),
        Some(0) => return Err(ctx.err("max_clients cannot be 0")),
        Some(n) => LruCache::new(std::num::NonZeroUsize::new(n).unwrap()),
    };

    let buffer_size = opts.buffer_size.unwrap_or(4096);

    let qlen = opts.qlen.unwrap_or(1);

    let backpressure = opts.backpressure;

    if buffer_size == 0 {
        return Err(ctx.err("Invalid buffer_size 0"));
    }

    debug!(parent: &span, addr=%opts.bind, "options parsed");

    let Some(Ok(s)) = UdpSocket::bind(bind_addr).now_or_never() else {
        return Err(ctx.err("Failed to bind UDP socket"));
    };

    let s = Arc::new(s);

    let mut buf = BytesMut::new();

    let mut clients_add_events: usize = 0;

    Ok(async move {
        debug!("node started");

        'main_loop: loop {
            trace!("loop");
            if clients_add_events == 1024 && opts.max_clients.unwrap_or(4096) >= 4096 {
                debug!("vacuum");
                let mut ctr = 0;
                let dead_clients = Vec::from_iter(
                    lru.iter()
                        .filter(|x| x.1.v.lock().unwrap().dead())
                        .map(|x| *x.0),
                );
                for x in dead_clients {
                    if lru.pop(&x).is_some() {
                        ctr += 1;
                    }
                }
                if ctr > 0 {
                    debug!("Vacuumed {ctr} stale entries");
                }
                clients_add_events = 0;
            }

            buf.reserve(buffer_size.saturating_sub(buf.capacity()));

            let (b, from_addr) = match s.recv_buf_from(&mut buf).await {
                Ok((n, from_addr)) => {
                    trace!(n, %from_addr, "recv");
                    let b = buf.split_to(n).freeze();
                    (b, from_addr)
                }
                Err(e) => {
                    error!("Error receiving from udp: {e}");
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    continue 'main_loop;
                }
            };

            let ci :&Arc<ClientInfo> = 'obtaining_entry: loop {
                trace!("lookup");
                break match lru.get(&from_addr) {
                    None => {
                        trace!("not found");
                        clients_add_events += 1;
                        let (tx, rx) = tokio::sync::mpsc::channel(qlen);
                        let (tx2, rx2) = tokio::sync::oneshot::channel();
                        let ci = Arc::new(ClientInfo {
                            addr: from_addr,
                            v: Mutex::new(VolatileClientInfo {
                                deadline: None,
                                removal_notifier: Some(tx2),
                                sink: tx,
                            }),
                        });
                        {
                            assert!(!ci.v.lock().unwrap().dead());
                        }
                        

                        let ci2 = ci.clone();
                        let ci3 = ci.clone();
                        if let Some((_, evicted)) = lru.push(from_addr, ci) {
                            debug!(peeraddr=%evicted.addr, "evicting");
                            let mut ev = evicted.v.lock().unwrap();
                            ev.terminate();
                        }

                        let udp_send = UdpSend::new(s.clone(), ci2, opts.inhibit_send_errors, opts.max_send_datagram_size);
                        let udp_recv = UdpRecv {
                            recv: rx,
                            tag_as_text: opts.tag_as_text,
                        };
                        let hangup =
                            Some(Box::pin(hangup_monitor(ci3, rx2)) as super::types::Hangup);
                        let socket = DatagramSocket {
                            read: Some(DatagramRead {
                                src: Box::pin(udp_recv),
                            }),
                            write: Some(DatagramWrite {
                                snk: Box::pin(udp_send),
                            }),
                            close: hangup,
                        };


                        let the_scenario = the_scenario.clone();
                        let continuation = continuation.clone();
                        tokio::spawn(async move {
                            let newspan = debug_span!("udp_accept", from=%from_addr);
                            debug!("accepted");
                            callback_and_continue::<(Handle<DatagramSocket>, SocketAddr)>(
                                the_scenario,
                                continuation,
                                (Some(socket).wrap(), from_addr),
                            )
                            .instrument(newspan)
                            .await;
                        });

                        lru.get(&from_addr).unwrap()
                    }
                    Some(x) => {
                        let dead = { x.v.lock().unwrap().dead() };
                        trace!(dead, "found");
                        if dead {
                            lru.pop(&from_addr);
                            continue 'obtaining_entry;
                        }
                        x
                    }
                };
            };

            let mut send_debt = None;
            {
                let mut v = ci.v.lock().unwrap();
                if v.dead() {
                    warn!("A rare case of a dropped incoming datagram because of timer expiration in an unfortunate moment.");
                    continue 'main_loop;
                }
                if let Some(tmo) = opts.timeout_ms {
                    let deadline = Instant::now() + Duration::from_millis(tmo);
                    v.deadline = Some(deadline);
                }

                match v.sink.try_send(b) {
                    Ok(()) => (),
                    Err(TrySendError::Closed(_)) => {
                        v.terminate();
                    }
                    Err(TrySendError::Full(b)) => {
                        if backpressure {
                            send_debt = Some((v.sink.clone(), b));
                            
                        } else {
                            debug!(peer_addr=%from_addr, "dropping a datagram due to handler being too slow")
                        }
                    }
                }
            }
            if let Some((sink2, b)) = send_debt {
                debug!(peer_addr=%from_addr, "buffer full, sending later");
                match sink2.send(b).await {
                    Ok(()) => (),
                    Err(_) => {
                        let mut vv = ci.v.lock().unwrap();
                        vv.terminate();
                    }
                }
            }
        }
    }
    .instrument(span)
    .wrap())
}

pub fn register(engine: &mut Engine) {
    engine.register_fn("udp_server", udp_server);
}
