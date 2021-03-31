extern crate tokio_codec;
extern crate websocket;
extern crate base64;

use self::websocket::stream::r#async::Stream as WsStream;
use self::websocket::OwnedMessage;
use futures;
use futures::sink::Sink;
use futures::stream::Stream;
use std;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use tokio_io::{AsyncRead, AsyncWrite};

use std::cell::RefCell;
use std::rc::Rc;

use futures::Async::{NotReady, Ready};

use super::{brokenpipe, io_other_error, wouldblock, Peer, HupToken};

use super::readdebt::{ProcessMessageResult, ReadDebt};

type MultiProducerWsSink<T> = Rc<
    RefCell<
        futures::stream::SplitSink<
            tokio_codec::Framed<T, websocket::r#async::MessageCodec<websocket::OwnedMessage>>,
        >,
    >,
>;
type WsSource<T> = futures::stream::SplitStream<
    tokio_codec::Framed<T, websocket::r#async::MessageCodec<websocket::OwnedMessage>>,
>;

pub struct WsReadWrapper<T: WsStream + 'static> {
    pub s: WsSource<T>,
    pub pingreply: MultiProducerWsSink<T>,
    pub debt: ReadDebt,
    pub pong_timeout: Option<(::tokio_timer::Delay, ::std::time::Duration)>,
    pub ping_aborter: Option<::futures::unsync::oneshot::Sender<()>>,

    pub text_prefix: Option<String>,
    pub binary_prefix: Option<String>,
    pub binary_base64: bool,
    pub text_base64: bool,
}

impl<T: WsStream + 'static> AsyncRead for WsReadWrapper<T> {}

impl<T: WsStream + 'static> Read for WsReadWrapper<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ret) = self.debt.check_debt(buf) {
            return ret;
        }
        macro_rules! abort_and_broken_pipe {
            () => {{
                if let Some(abt) = self.ping_aborter.take() {
                    let _ = abt.send(());
                }
                brokenpipe()
            }};
        }
        fn process_prefixes_and_base64<'a>(qbuf :&'a mut Vec<u8>, q: &mut &'a [u8], prefix: &Option<String>, base64: bool) {
            match (prefix, base64) {
                (None, false) => (),
                (Some(pr), false) => {
                    debug!("prepending prefix");
                    qbuf.reserve_exact(pr.len() + q.len());
                    qbuf.extend_from_slice(pr.as_bytes());
                    qbuf.extend_from_slice(q);
                    *q = &mut qbuf[..];
                }
                (None, true) => {
                    debug!("encoding to base64");
                    qbuf.resize(q.len() * 3 / 2 + 3, 0);
                    let r = base64::encode_config_slice(q, base64::STANDARD, &mut qbuf[..]);
                    qbuf.resize(r, 0);
                    qbuf.push(b'\n');
                    *q = &mut qbuf[..];
                },
                (Some(pr), true) => {
                    debug!("prepending prefix and encoding to base64");
                    qbuf.extend_from_slice(pr.as_bytes());
                    qbuf.resize(pr.len() + q.len() * 3 / 2 + 3, 0);
                    let r = base64::encode_config_slice(q, base64::STANDARD, &mut qbuf[pr.len()..]);
                    qbuf.resize(pr.len()+r, 0);
                    qbuf.push(b'\n');
                    *q = &mut qbuf[..];
                },
            }
        }
        loop {
            return match self.s.poll().map_err(io_other_error)? {
                Ready(Some(OwnedMessage::Close(x))) => {
                    info!("Received WebSocket close message");
                    debug!("The close message is {:?}", x);
                    abort_and_broken_pipe!()
                }
                Ready(None) => {
                    info!("incoming None");
                    abort_and_broken_pipe!()
                }
                Ready(Some(OwnedMessage::Ping(x))) => {
                    info!("Received WebSocket ping");
                    let om = OwnedMessage::Pong(x);
                    let mut sink = self.pingreply.borrow_mut();
                    let mut proceed = false;
                    // I'm not sure this is safe enough, RefCell-wise and Futures-wise
                    // And pings and their replies are not tested yet
                    match sink.start_send(om).map_err(io_other_error)? {
                        futures::AsyncSink::NotReady(_) => {
                            warn!(
                                "dropped a ping request from websocket due to channel contention"
                            );
                        }
                        futures::AsyncSink::Ready => {
                            proceed = true;
                        }
                    }
                    if proceed {
                        let _ = sink.poll_complete().map_err(io_other_error)?;
                    }

                    continue;
                }
                Ready(Some(OwnedMessage::Pong(_))) => {
                    info!("Received a pong from websocket");

                    if let Some((de, intvl)) = self.pong_timeout.as_mut() {
                        de.reset(::std::time::Instant::now() + *intvl);
                    }
                    continue;
                }
                Ready(Some(OwnedMessage::Text(x))) => {
                    debug!("incoming text");
                    let mut qbuf : Vec<u8> = vec![];
                    let mut q : &[u8] = x.as_str().as_bytes();
                    process_prefixes_and_base64(&mut qbuf, &mut q, &self.text_prefix, self.text_base64);
                    match self.debt.process_message(buf, q) {
                        ProcessMessageResult::Return(x) => x,
                        ProcessMessageResult::Recurse => continue,
                    }
                }
                Ready(Some(OwnedMessage::Binary(x))) => {
                    debug!("incoming binary");
                    let mut qbuf : Vec<u8> = vec![];
                    let mut q : &[u8] = x.as_slice();
                    process_prefixes_and_base64(&mut qbuf, &mut q, &self.binary_prefix, self.binary_base64);
                    match self.debt.process_message(buf, q) {
                        ProcessMessageResult::Return(x) => x,
                        ProcessMessageResult::Recurse => continue,
                    }
                }
                NotReady => {
                    use futures::Async;
                    use futures::Future;
                    if let Some((de, _intvl)) = self.pong_timeout.as_mut() {
                        match de.poll() {
                            Err(e) => error!("tokio-timer's Delay: {}", e),
                            Ok(Async::NotReady) => (),
                            Ok(Async::Ready(_inst)) => {
                                warn!("Closing WebSocket connection due to ping timeout");
                                return abort_and_broken_pipe!();
                            }
                        }
                    }
                    wouldblock()
                }
            };
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Mode1 {
    Text,
    Binary,
}

pub struct WsWriteWrapper<T: WsStream + 'static> {
    pub sink: MultiProducerWsSink<T>,
    pub mode: Mode1,
    pub close_on_shutdown: bool,

    pub text_prefix: Option<String>,
    pub binary_prefix: Option<String>,
    pub binary_base64: bool,
    pub text_base64: bool,
}

impl<T: WsStream + 'static> AsyncWrite for WsWriteWrapper<T> {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        if !self.close_on_shutdown {
            return Ok(Ready(()));
        }
        let mut sink = self.sink.borrow_mut();
        match sink
            .start_send(OwnedMessage::Close(None))
            .map_err(io_other_error)?
        {
            futures::AsyncSink::NotReady(_) => wouldblock(),
            futures::AsyncSink::Ready => {
                // Too lazy to implement a state machine here just for
                // properly handling this.
                // And shutdown result is ignored here anyway.
                let _ = sink.poll_complete().map_err(|_| ()).map(|_| ());
                Ok(Ready(()))
            }
        }
    }
}

impl<T: WsStream + 'static> Write for WsWriteWrapper<T> {
    fn write(&mut self, buf_: &[u8]) -> IoResult<usize> {
        let bufv;
        let mut effective_mode = self.mode;

        let mut buf : &[u8] = buf_;

        let origlen = buf.len();

        if let Some(pr) = &self.text_prefix {
            if buf.starts_with(pr.as_bytes()) {
                effective_mode = Mode1::Text;
                buf = &buf[pr.len()..];
            }
        }
        if let Some(pr) = &self.binary_prefix {
            if buf.starts_with(pr.as_bytes()) {
                effective_mode = Mode1::Binary;
                buf = &buf[pr.len()..];
            }
        }

        let decode_base64 = match effective_mode {
            Mode1::Binary => self.binary_base64,
            Mode1::Text => self.text_base64,
        };

        if decode_base64 {
            if buf.last() == Some(&b'\n') {
                buf = &buf[..(buf.len()-1)];
            }
            if buf.last() == Some(&b'\r') {
                buf = &buf[..(buf.len()-1)];
            }
            if let Ok(v) = base64::decode(buf) {
                bufv = v;
                buf = &bufv[..];
            } else {
                error!("Failed to decode user-supplised base64 buffer. Sending message as is.");
            }
        }

        let om = match effective_mode {
            Mode1::Binary => OwnedMessage::Binary(buf.to_vec()),
            Mode1::Text => {
                let text_tmp;
                let text = match ::std::str::from_utf8(buf) {
                    Ok(x) => x,
                    Err(_) => {
                        error!(
                            "Invalid UTF-8 in a text WebSocket message. Sending lossy data. May be \
                             caused by unlucky buffer splits."
                        );
                        text_tmp = String::from_utf8_lossy(buf);
                        text_tmp.as_ref()
                    }
                };
                OwnedMessage::Text(text.to_string())
            }
        };
        match self.sink.borrow_mut().start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => wouldblock(),
            futures::AsyncSink::Ready => Ok(origlen),
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self
            .sink
            .borrow_mut()
            .poll_complete()
            .map_err(io_other_error)?
        {
            NotReady => wouldblock(),
            Ready(()) => Ok(()),
        }
    }
}

impl<T: WsStream + 'static> Drop for WsWriteWrapper<T> {
    fn drop(&mut self) {
        debug!("drop WsWriteWrapper",);
        // moved to shutdown()
    }
}

pub struct PeerForWs(pub Peer);

//implicit impl websocket::stream::async::Stream for PeerForWs {}

impl AsyncRead for PeerForWs {}
impl Read for PeerForWs {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        (self.0).0.read(buf)
    }
}
impl AsyncWrite for PeerForWs {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        (self.0).1.shutdown()
    }
}
impl Write for PeerForWs {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        (self.0).1.write(buf)
    }
    fn flush(&mut self) -> IoResult<()> {
        (self.0).1.flush()
    }
}

enum WsPingerState {
    WaitingForTimer,
    StartSend,
    PollComplete,
}

/// Periodically sends WebSocket pings
pub struct WsPinger<T: WsStream + 'static> {
    st: WsPingerState,
    si: MultiProducerWsSink<T>,
    t: ::tokio_timer::Interval,
    aborter: ::futures::unsync::oneshot::Receiver<()>,
}

impl<T: WsStream + 'static> WsPinger<T> {
    pub fn new(
        sink: MultiProducerWsSink<T>,
        interval: ::std::time::Duration,
        aborter: ::futures::unsync::oneshot::Receiver<()>,
    ) -> Self {
        WsPinger {
            st: WsPingerState::WaitingForTimer,
            t: ::tokio_timer::Interval::new_interval(interval),
            si: sink,
            aborter,
        }
    }
}

impl<T: WsStream + 'static> ::futures::Future for WsPinger<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> ::futures::Poll<(), ()> {
        use self::WsPingerState::*;
        use futures::Async;
        use futures::AsyncSink;
        loop {
            match self.aborter.poll() {
                Err(e) => warn!("unsync/oneshot: {}", e),
                Ok(Async::NotReady) => (),
                Ok(Async::Ready(())) => {
                    debug!("Pinger aborted");
                    return Ok(Async::Ready(()));
                }
            }
            match self.st {
                WaitingForTimer => match self.t.poll() {
                    Err(e) => warn!("wspinger: {}", e),
                    Ok(Async::Ready(None)) => warn!("tokio-timer's interval stream ended?"),
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Ok(Async::Ready(Some(_instant))) => {
                        self.st = StartSend;
                        info!("Sending WebSocket ping");
                        continue;
                    }
                },
                StartSend => {
                    let om = OwnedMessage::Ping(vec![]);
                    match self.si.borrow_mut().start_send(om) {
                        Err(e) => info!("wsping: {}", e),
                        Ok(AsyncSink::NotReady(_om)) => {
                            return Ok(Async::NotReady);
                        }
                        Ok(AsyncSink::Ready) => {
                            self.st = PollComplete;
                            continue;
                        }
                    }
                }
                PollComplete => match self.si.borrow_mut().poll_complete() {
                    Err(e) => info!("wsping: {}", e),
                    Ok(Async::NotReady) => {
                        return Ok(Async::NotReady);
                    }
                    Ok(Async::Ready(())) => {
                        self.st = WaitingForTimer;
                        continue;
                    }
                },
            }
            return Ok(Async::Ready(()));
        }
    }
}


pub type Duplex<S> = ::tokio_codec::Framed<S, websocket::r#async::MessageCodec<websocket::OwnedMessage>>;

pub fn finish_building_ws_peer<S>(opts: &super::Options, duplex: Duplex<S>, close_on_shutdown: bool, hup: Option<HupToken>) -> Peer
    where S : tokio_io::AsyncRead + tokio_io::AsyncWrite + 'static + Send
{
    let (sink, stream) = duplex.split();
    let mpsink = Rc::new(RefCell::new(sink));

    let mode1 = if opts.websocket_text_mode {
        Mode1::Text
    } else {
        Mode1::Binary
    };

    let ping_aborter = if let Some(d) = opts.ws_ping_interval {
        debug!("Starting pinger");

        let (tx, rx) = ::futures::unsync::oneshot::channel();

        let intv = ::std::time::Duration::from_secs(d);
        let pinger = super::ws_peer::WsPinger::new(mpsink.clone(), intv, rx);
        ::tokio_current_thread::spawn(pinger);
        Some(tx)
    } else {
        None
    };

    let pong_timeout = if let Some(d) = opts.ws_ping_timeout {
        let to = ::std::time::Duration::from_secs(d);
        let de = ::tokio_timer::Delay::new(std::time::Instant::now() + to);
        Some((de, to))
    } else {
        None
    };

    let zmsgh = if opts.no_exit_on_zeromsg {
        super::readdebt::ZeroMessagesHandling::Drop
    } else {
        super::readdebt::ZeroMessagesHandling::Deliver
    };
    
    let ws_str = WsReadWrapper {
        s: stream,
        pingreply: mpsink.clone(),
        debt: super::readdebt::ReadDebt(Default::default(), opts.read_debt_handling, zmsgh),
        pong_timeout,
        ping_aborter,
        text_prefix: opts.ws_text_prefix.clone(),
        binary_prefix: opts.ws_binary_prefix.clone(),
        binary_base64: opts.ws_binary_base64,
        text_base64: opts.ws_text_base64,
    };
    let ws_sin = WsWriteWrapper{
        sink: mpsink,
        mode: mode1,
        close_on_shutdown,

        text_prefix: opts.ws_text_prefix.clone(),
        binary_prefix: opts.ws_binary_prefix.clone(),
        binary_base64: opts.ws_binary_base64,
        text_base64: opts.ws_text_base64,
    };

    Peer::new(ws_str, ws_sin, hup)
}
