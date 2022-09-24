
#![allow(unused)]
#![cfg_attr(feature="cargo-clippy",allow(needless_pass_by_value,cast_lossless,identity_op))]
use futures::future::{err, ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_strerr, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use tokio_io::io::{read_exact, write_all};
use tokio_io::{AsyncRead,AsyncWrite};

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

use std::ffi::OsString;

extern crate http_bytes;
use http_bytes::http;

use http_bytes::{Request,Response};
use crate::http::Uri;
use crate::http::Method;
use crate::util::peer_err2;

#[derive(Debug)]
pub struct HttpRequest<T: Specifier>(pub T);
impl<T: Specifier> Specifier for HttpRequest<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            let mut b = crate::http::request::Builder::default();
            if let Some(uri) = cp.program_options.request_uri.as_ref() {
                b.uri(uri);
            }
            if let Some(method) = cp.program_options.request_method.as_ref() {
                b.method(method);
            }
            for (hn, hv) in &cp.program_options.request_headers {
                b.header(hn, hv);
            }
            let request = b.body(()).unwrap();
            http_request_peer(&request, p, l2r)
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = HttpRequestClass,
    target = HttpRequest,
    prefixes = ["http-request:"],
    arg_handling = subspec,
    overlay = true,
    StreamOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
the torch to outer peer, if any - lowlevel version.

Content you write becomes body, content you read is body that server has sent.

URI is specified using a separate command-line parameter

Example:

    websocat -Ub - http-request:tcp:example.com:80 --request-uri=http://example.com/ --request-header 'Connection: close'
"#
);

/// Inner peer is a TCP peer configured to this host
#[derive(Debug)]
pub struct Http<T: Specifier>(pub T, pub Uri);
impl<T: Specifier> Specifier for Http<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        let uri = self.1.clone();
        inner.map(move |p, l2r| {
            let mut b = crate::http::request::Builder::default();
            b.uri(uri.clone());
            if let Some(method) = cp.program_options.request_method.as_ref() {
                b.method(method);
            }
            for (hn, hv) in &cp.program_options.request_headers {
                b.header(hn, hv);
            }
            let request = b.body(()).unwrap();
            http_request_peer(&request, p, l2r)
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = HttpClass,
    target = Http,
    prefixes = ["http:"],
    arg_handling = {
        fn construct(self: &HttpClass, arg: &str) -> super::Result<Rc<dyn Specifier>> {
            let uri : Uri = format!("http:{}", arg).parse()?;
            let tcp_peer;
            {
                let auth = uri.authority_part().unwrap();
                let host = auth.host();
                let port = auth.port_part();
                let addr = if let Some(p) = port {
                    format!("tcp:{}:{}", host, p)
                } else {
                    format!("tcp:{}:80", host)
                };
                tcp_peer = crate::spec(addr.as_ref())?;
            }
            Ok(Rc::new(Http(tcp_peer, uri)))
        }
        fn construct_overlay(
            self: &HttpClass,
            _inner: Rc<dyn Specifier>,
        ) -> super::Result<Rc<dyn Specifier>> {
            panic!("Error: construct_overlay called on non-overlay specifier class")
        }
    },
    overlay = false,
    StreamOriented,
    SingleConnect,
    help = r#"
[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
the torch to outer peer, if any - highlevel version.

Content you write becomes body, content you read is body that server has sent.

URI is specified inline.

Example:

    websocat  -b - http://example.com < /dev/null
"#
);



#[derive(Copy,Clone,PartialEq,Debug)]
enum HttpHeaderEndDetectionState {
    Neutral,
    FirstCr,
    FirstLf,
    SecondCr,
    FoundHeaderEnd,
}

struct WaitForHttpHead<R : AsyncRead>
{
    buf: Option<Vec<u8>>,
    offset : usize,
    state: HttpHeaderEndDetectionState,
    io : Option<R>,
}

struct WaitForHttpHeadResult {
    buf: Vec<u8>,
    // Before the offset is header, after the offset is debt
    offset: usize,
}

impl<R:AsyncRead> WaitForHttpHead<R> {
    pub fn new(r:R) -> WaitForHttpHead<R> {
        WaitForHttpHead {
            buf: Some(Vec::with_capacity(512)),
            offset: 0,
            state: HttpHeaderEndDetectionState::Neutral,
            io: Some(r),
        }
    }
}

impl<R:AsyncRead> Future for WaitForHttpHead<R> {
    type Item = (WaitForHttpHeadResult, R);
    type Error = Box<dyn std::error::Error>;

    fn poll(&mut self) -> ::futures::Poll<Self::Item, Self::Error> {
        loop {
            if self.buf.is_none() || self.io.is_none() {
                Err("WaitForHttpHeader future polled after completion")?;
            }
            let ret;
            {
                let buf = self.buf.as_mut().unwrap();
                let io = self.io.as_mut().unwrap();
                if buf.len() < self.offset + 1024 {
                    buf.resize(self.offset + 1024, 0u8);
                }
                ret = try_nb!(io.read(&mut buf[self.offset..]));

                if ret == 0 {
                    Err("Trimmed HTTP head")?;
                }
            }

            // parse
            for i in self.offset..(self.offset+ret) {
                let x = self.buf.as_ref().unwrap()[i];
                use self::HttpHeaderEndDetectionState::*;
                //eprint!("{:?} -> ", self.state);
                self.state = match (self.state, x) {
                    (Neutral, b'\r') => FirstCr,
                    (FirstCr, b'\n') => FirstLf,
                    (FirstLf, b'\r') => SecondCr,
                    (SecondCr, b'\n') => FoundHeaderEnd,
                    _ => Neutral,
                };
                //eprintln!("{:?}", self.state);
                if self.state == FoundHeaderEnd {
                    let io = self.io.take().unwrap();
                    let mut buf = self.buf.take().unwrap();
                    buf.resize(self.offset + ret, 0u8);
                    return Ok(::futures::Async::Ready((
                        WaitForHttpHeadResult { buf, offset: i+1},
                        io,
                    )));
                }
            }

            self.offset += ret;

            if self.offset > 60_000 {
                Err("HTTP head too long")?;
            }
        }
    }
}

pub fn http_request_peer(
    request: &Request,
    inner_peer: Peer,
    _l2r: L2rUser,
) -> BoxedNewPeerFuture {
    let request = ::http_bytes::request_header_to_vec(&request);

    let (r, w, hup) = (inner_peer.0, inner_peer.1, inner_peer.2);

    info!("Issuing HTTP request");
    let f = ::tokio_io::io::write_all(w, request)
        .map_err(box_up_err)
        .and_then(move |(w, request)| {
            WaitForHttpHead::new(r).and_then(|(res, r)|{
                debug!("Got HTTP response head");
                let ret = (move||{
                    {
                        let headbuf = &res.buf[0..res.offset];
                        trace!("{:?}",headbuf);
                        let p = http_bytes::parse_response_header_easy(headbuf)?;
                        if p.is_none() {
                            Err("Something wrong with response HTTP head")?;
                        }
                        let p = p.unwrap();
                        if p.1.len() > 0 {
                            Err("Something wrong with parsing HTTP")?;
                        }
                        let response = p.0;
                        let status = response.status();
                        info!("HTTP response status: {}", status);
                        debug!("{:#?}", response);
                        if status.is_success() || status.is_informational() {
                            // OK
                        } else {
                            Err("HTTP response indicates failure")?;
                        }
                    }
                    let remaining = res.buf.len() - res.offset;
                    if remaining == 0 {
                        Ok(Peer::new(r,w,hup))
                    } else {
                        debug!("{} bytes of debt to be read", remaining);
                        let r = super::trivial_peer::PrependRead {
                            inner: r,
                            header: res.buf,
                            remaining,
                        };
                        Ok(Peer::new(r,w,hup))
                    }
                })();
                ::futures::future::result(ret)
            })
        })
    ;

    Box::new(f) as BoxedNewPeerFuture
}


#[derive(Debug)]
pub struct HttpPostSse<T: Specifier>(pub T);
impl<T: Specifier> Specifier for HttpPostSse<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            http_response_post_sse_peer(p, l2r)
        })
    }
    specifier_boilerplate!(noglobalstate has_subspec);
    self_0_is_subspecifier!(proxy_is_multiconnect);
}
specifier_class!(
    name = HttpPostSseClass,
    target = HttpPostSse,
    prefixes = ["http-post-sse:"],
    arg_handling = subspec,
    overlay = true,
    MessageOriented,
    MulticonnectnessDependsOnInnerType,
    help = r#"
[A] Accept HTTP/1 request. Then, if it is GET,
unidirectionally return incoming messages as server-sent events (SSE).

If it is POST then, also unidirectionally, write body upstream.

Example - turn SSE+POST pair into a client WebSocket connection:

    websocat -E -t http-post-sse:tcp-l:127.0.0.1:8080 reuse:ws://127.0.0.1:80/websock

`curl -dQQQ http://127.0.0.1:8080/` would send into it and 
`curl -N http://127.0.0.1:8080/` would recv from it.
"#
);

#[derive(Debug)]
enum ModeOfOperation {
    PostBody,
    GetSse,
}

pub fn http_response_post_sse_peer(
    inner_peer: Peer,
    _l2r: L2rUser,
) -> BoxedNewPeerFuture {
    let (r, w, hup) = (inner_peer.0, inner_peer.1, inner_peer.2);

    warn!("Note: http-post-see mode is not tested and may integrate poorly into current Websocat architecture. Expect it to be of lower quality than other Websocat modes.");
    info!("Incoming prospective HTTP request");
    let f = WaitForHttpHead::new(r).and_then(|(res, r)|{
        debug!("Got HTTP request head");
        let ret : Result<_,Box<dyn std::error::Error+'static>> = (move||{
            let mode;
            let request;
            {
                let headbuf = &res.buf[0..res.offset];
                trace!("{:?}",headbuf);
                let p = http_bytes::parse_request_header_easy(headbuf)?;
                if p.is_none() {
                    Err("Something wrong with request HTTP head")?;
                }
                let p = p.unwrap();
                if p.1.len() > 0 {
                    Err("Something wrong with parsing HTTP request")?;
                }
                request = p.0;
                let method = request.method();
                mode = match *method {
                    http::method::Method::GET => {
                        info!("GET request. Serving a SSE stream");
                        ModeOfOperation::GetSse
                    },
                    http::method::Method::POST => {
                        info!("POST request. Writing body once");
                        ModeOfOperation::PostBody
                    },
                    _ => { 
                        error!("HTTP request method is {}, but we expect only GET or POST in this mode", method);
                        Err("Wrong HTTP request method")?
                    }
                };
                debug!("{:#?}", request);
            }

            // Now it's time to generate a successful reply.
            // (maybe actually we should read the full request first, but
            // let's do some thing at first and correct thing after that).

            use crate::http::header::{HOST,SERVER,CACHE_CONTROL, CONTENT_TYPE};

            let mut reply = crate::http::response::Builder::default();
            let status = match mode {
                ModeOfOperation::GetSse => 200,
                ModeOfOperation::PostBody => 204,
            };
            reply.status(status);
            if let Some(x) = request.headers().get(HOST) {
                reply.header(HOST, x);
            }
            reply.header("Server", "websocat");
            match mode {
                ModeOfOperation::GetSse => {
                    reply.header(CACHE_CONTROL, "no-cache");
                    reply.header(CONTENT_TYPE, "text/event-stream");
                }
                ModeOfOperation::PostBody => (),
            }
            let reply = reply.body(()).unwrap();
            let reply = ::http_bytes::response_header_to_vec(&reply);

            Ok(::tokio_io::io::write_all(w, reply)
                .map_err(box_up_err)
                .and_then(move |(w, request)| {

                    debug!("Response writing finished");

                    // Infinitely hang reading or writing
                    // If use DevNull instead, connection may get closed prematurely
                    let dummy = crate::trivial_peer::CloggedPeer;

                    match mode {
                        ModeOfOperation::GetSse => {
                            // Will it call shutdown(2) on the socket?
                            drop(r);
                            
                            let w = SseStream::new(w);
                            
                            Ok(Peer::new(dummy, w, hup))
                        },
                        ModeOfOperation::PostBody => {
                            debug!("Start streaming POST body upstream, ignoring reverse data");
                            
                            // Will it call shutdown(2) on the socket?
                            drop(w);

                            let remaining = res.buf.len() - res.offset;
                            if remaining == 0 {
                                Ok(Peer::new(r,dummy,hup))
                            } else {
                                debug!("{} bytes of debt to be read", remaining);
                                let r = super::trivial_peer::PrependRead {
                                    inner: r,
                                    header: res.buf,
                                    remaining,
                                };
                                Ok(Peer::new(r,dummy,hup))
                            }
                        },
                    }
            }))
        })();
        match ret {
            Err(x) => peer_err2(x),
            Ok(x) => Box::new(x),
        }
    });
    Box::new(f) as BoxedNewPeerFuture
}

#[derive(Clone,Copy,Debug)]
enum SseState {
    BeforeLine(usize),
    InsideLine,
    AfterLine,
    Trailer,
}

struct SseStream<W : Write>
{
    io : W,
    state: SseState,
    consumed_actual_buffer : usize,
}

impl<W : Write> SseStream<W> {
    pub fn new(w: W) -> Self {
        SseStream {
            io: w,
            state: SseState::BeforeLine(0),
            consumed_actual_buffer: 0,
        }
    }
}

impl<W:AsyncWrite> AsyncWrite for SseStream<W> {
    fn shutdown(&mut self) -> futures::Poll<(), std::io::Error> {
        self.io.shutdown()
    }
}

impl<W:AsyncWrite> Write for SseStream<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // assumes buffer does not change on EAGAINs
        loop {
            let s = self.state;
            let mut need_write = 0;
            debug!("SSE state {:?}", s);
            let ret = match s {
                SseState::BeforeLine(x) => {
                    self.io.write(&b"data: "[x..6])
                }
                SseState::InsideLine => {
                    let buf = &buf[self.consumed_actual_buffer..];
                    let max = buf.iter().position(|&x|x==b'\n').unwrap_or(buf.len());
                    let buf = &buf[0..max];
                    need_write = buf.len();
                    self.io.write(buf)
                }
                SseState::AfterLine => {
                    self.io.write(b"\n")
                }
                SseState::Trailer => {
                    self.io.write(b"\n")
                }
            };
            let ll = ret?;
            self.state = match s {
                SseState::BeforeLine(x) => {
                    let nl = ll + x;
                    if nl == 6 {
                        SseState::InsideLine
                    } else {
                        SseState::BeforeLine(nl)
                    }
                }
                SseState::InsideLine => {
                    self.consumed_actual_buffer += ll;
                    if ll < need_write {
                        SseState::InsideLine
                    } else {
                        SseState::AfterLine
                    }
                }
                SseState::AfterLine => {
                    if self.consumed_actual_buffer < buf.len() {
                        if buf[self.consumed_actual_buffer] == b'\n' {
                            self.consumed_actual_buffer += 1;
                        }
                        SseState::BeforeLine(0)
                    } else {
                        SseState::Trailer
                    }
                }
                SseState::Trailer => {
                    let r = self.consumed_actual_buffer;
                    self.consumed_actual_buffer = 0;
                    self.state = SseState::BeforeLine(0);
                    debug!("r={} buflen={}", r, buf.len());
                    return Ok(r)
                }
            };
            debug!(" new SSE state {:?}", self.state);
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.io.flush()
    }
}

#[test]
fn test_basic_sse_stream() {
    let mut v = vec![];
    {
        let mut ss = SseStream::new(std::io::Cursor::new(&mut v));
    }
}