
#![allow(unused)]
#![cfg_attr(feature="cargo-clippy",allow(needless_pass_by_value,cast_lossless,identity_op))]
use futures::future::{err, ok, Future};

use std::rc::Rc;

use super::{box_up_err, peer_strerr, BoxedNewPeerFuture, Peer};
use super::{ConstructParams, L2rUser, PeerConstructor, Specifier};
use tokio_io::io::{read_exact, write_all};
use tokio_io::AsyncRead;

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};

use std::ffi::OsString;

extern crate http_bytes;
use http_bytes::http;

use http_bytes::{Request,Response};
use http::Uri;
use http::Method;

#[derive(Debug)]
pub struct HttpRequest<T: Specifier>(pub T);
impl<T: Specifier> Specifier for HttpRequest<T> {
    fn construct(&self, cp: ConstructParams) -> PeerConstructor {
        let inner = self.0.construct(cp.clone());
        inner.map(move |p, l2r| {
            let mut b = ::http::request::Builder::default();
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
the torch to outer peer, if any.

Content you write becomes body, content you read is body that server has sent.

URI is specified using a separate command-line parameter

Example:

    websocat -Ub - http-request:tcp:example.com:80 --request-uri=http://example.com/ --request-header 'Connection: close'
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
            let buf = self.buf.as_mut().unwrap();
            let io = self.io.as_mut().unwrap();
            if buf.len() < self.offset + 1024 {
                buf.resize(self.offset + 1024, 0u8);
            }
            let ret = try_nb!(io.read(&mut buf[self.offset..]));

            if ret == 0 {
                Err("Trimmed HTTP head")?;
            }

            // parse
            for i in self.offset..(self.offset+ret) {
                let x = buf[i];
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
                    drop((buf,io));
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

    let (r, w) = (inner_peer.0, inner_peer.1);

    info!("Issuing HTTP request");
    let f = ::tokio_io::io::write_all(w, request)
        .map_err(box_up_err)
        .and_then(move |(w, request)| {
            WaitForHttpHead::new(r).and_then(|(res, r)|{
                debug!("Got HTTP response head");
                let ret = (move||{
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
                    let remaining = res.buf.len() - res.offset;
                    if remaining == 0 {
                        Ok(Peer::new(r,w))
                    } else {
                        debug!("{} bytes of debt to be read", remaining);
                        let r = super::trivial_peer::PrependRead {
                            inner: r,
                            header: res.buf,
                            remaining,
                        };
                        Ok(Peer::new(r,w))
                    }
                })();
                ::futures::future::result(ret)
            })
        })
    ;

    Box::new(f) as BoxedNewPeerFuture
}
