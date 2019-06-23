extern crate tokio_codec;
extern crate websocket;

use self::websocket::stream::async::Stream as WsStream;
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
            (tokio_codec::Framed<T, websocket::async::MessageCodec<websocket::OwnedMessage>>),
        >,
    >,
>;
type WsSource<T> = futures::stream::SplitStream<
    tokio_codec::Framed<T, websocket::async::MessageCodec<websocket::OwnedMessage>>,
>;

pub struct WsReadWrapper<T: WsStream + 'static> {
    pub s: WsSource<T>,
    pub pingreply: MultiProducerWsSink<T>,
    pub debt: ReadDebt,
    pub pong_timeout: Option<(::tokio_timer::Delay, ::std::time::Duration)>,
    pub ping_aborter: Option<::futures::unsync::oneshot::Sender<()>>,
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
        loop {
            return match self.s.poll().map_err(io_other_error)? {
                Ready(Some(OwnedMessage::Close(_))) => {
                    debug!("incoming close");
                    abort_and_broken_pipe!()
                }
                Ready(None) => {
                    debug!("incoming None");
                    abort_and_broken_pipe!()
                }
                Ready(Some(OwnedMessage::Ping(x))) => {
                    debug!("incoming ping");
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
                    match self.debt.process_message(buf, x.as_str().as_bytes()) {
                        ProcessMessageResult::Return(x) => x,
                        ProcessMessageResult::Recurse => continue,
                    }
                }
                Ready(Some(OwnedMessage::Binary(x))) => {
                    debug!("incoming binary");
                    match self.debt.process_message(buf, x.as_slice()) {
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

pub struct WsWriteWrapper<T: WsStream + 'static>(pub MultiProducerWsSink<T>, pub Mode1, pub bool);

impl<T: WsStream + 'static> AsyncWrite for WsWriteWrapper<T> {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        if !self.2 {
            return Ok(Ready(()));
        }
        let mut sink = self.0.borrow_mut();
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
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let om = match self.1 {
            Mode1::Binary => OwnedMessage::Binary(buf.to_vec()),
            Mode1::Text => {
                let text_tmp;
                let text = match ::std::str::from_utf8(buf) {
                    Ok(x) => x,
                    Err(_) => {
                        error!(
                            "Invalid UTF-8 in --text mode. Sending lossy data. May be \
                             caused by unlucky buffer splits."
                        );
                        text_tmp = String::from_utf8_lossy(buf);
                        text_tmp.as_ref()
                    }
                };
                OwnedMessage::Text(text.to_string())
            }
        };
        match self.0.borrow_mut().start_send(om).map_err(io_other_error)? {
            futures::AsyncSink::NotReady(_) => wouldblock(),
            futures::AsyncSink::Ready => Ok(buf.len()),
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        match self
            .0
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


pub type Duplex<S> = ::tokio_codec::Framed<S, websocket::async::MessageCodec<websocket::OwnedMessage>>;

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

    let ws_str = WsReadWrapper {
        s: stream,
        pingreply: mpsink.clone(),
        debt: super::readdebt::ReadDebt(Default::default(), opts.read_debt_handling),
        pong_timeout,
        ping_aborter,
    };
    let ws_sin = WsWriteWrapper(mpsink, mode1, close_on_shutdown);

    Peer::new(ws_str, ws_sin, hup)
}
