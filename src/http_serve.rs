use self::hyper::http::h1::Incoming;
use self::hyper::method::Method;
use self::hyper::uri::RequestUri;
use self::hyper::uri::RequestUri::AbsolutePath;
use super::hyper;

use futures::future::Future;
use std::fs::File;
use std::rc::Rc;

use crate::options::StaticFile;
use crate::trivial_peer::get_literal_peer_now;
use crate::Peer;

use crate::my_copy::{copy, CopyOptions};

const BAD_REQUEST :&[u8] = b"HTTP/1.1 400 Bad Request\r\nServer: websocat\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nOnly WebSocket connections are welcome here\n";

const NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\nServer: websocat\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nURI does not match any -F option and is not a WebSocket connection.\n";

const NOT_FOUND2: &[u8] = b"HTTP/1.1 500 Not Found\r\nServer: websocat\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nFailed to open the file on server side.\n";

const BAD_METHOD :&[u8] = b"HTTP/1.1 400 Bad Request\r\nServer: websocat\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nHTTP method should be GET\n";

const BAD_URI_FORMAT :&[u8] = b"HTTP/1.1 400 Bad Request\r\nServer: websocat\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nURI should be an absolute path\n";

pub fn get_static_file_reply(len: Option<u64>, ct: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(256);
    q.extend_from_slice(b"HTTP/1.1 200 OK\r\nServer: websocat\r\nContent-Type: ");
    q.extend_from_slice(ct.as_bytes());
    q.extend_from_slice(b"\r\n");
    if let Some(x) = len {
        q.extend_from_slice(b"Content-Length: ");
        q.extend_from_slice(format!("{}", x).as_bytes());
        q.extend_from_slice(b"\r\n");
    }
    q.extend_from_slice(b"\r\n");
    q
}

#[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
pub fn http_serve(
    p: Peer,
    incoming: Option<Incoming<(Method, RequestUri)>>,
    serve_static_files: Rc<Vec<StaticFile>>,
) -> Box<dyn Future<Item = (), Error = ()>> {
    let mut serve_file = None;
    let content = if serve_static_files.is_empty() {
        BAD_REQUEST.to_vec()
    } else if let Some(inc) = incoming {
        info!("HTTP-serving {:?}", inc.subject);
        if inc.subject.0 == Method::Get {
            match inc.subject.1 {
                AbsolutePath(x) => {
                    let mut reply = None;
                    for sf in &*serve_static_files {
                        if sf.uri == x {
                            match File::open(&sf.file) {
                                Ok(f) => {
                                    let fs = match f.metadata() {
                                        Err(_) => None,
                                        Ok(x) => Some(x.len()),
                                    };
                                    reply = Some(get_static_file_reply(fs, &sf.content_type));
                                    serve_file = Some(f);
                                }
                                Err(_) => {
                                    reply = Some(NOT_FOUND2.to_vec());
                                }
                            }
                        }
                    }
                    reply.unwrap_or_else(|| NOT_FOUND.to_vec())
                }
                _ => BAD_URI_FORMAT.to_vec(),
            }
        } else {
            BAD_METHOD.to_vec()
        }
    } else {
        BAD_REQUEST.to_vec()
    };
    let reply = get_literal_peer_now(content);

    let co = CopyOptions {
        buffer_size: 1024,
        once: false,
        stop_on_reader_zero_read: true,
        skip: false,
        max_ops: None,
    };

    if let Some(f) = serve_file {
        Box::new(
            copy(reply, p.1, co, vec![])
                .map_err(drop)
                .and_then(move |(_len, _, conn)| {
                    let co2 = CopyOptions {
                        buffer_size: 65536,
                        once: false,
                        stop_on_reader_zero_read: true,
                        skip: false,
                        max_ops: None,
                    };
                    let wr = crate::file_peer::ReadFileWrapper(f);
                    copy(wr, conn, co2, vec![]).map(|_| ()).map_err(drop)
                }),
        )
    } else {
        Box::new(copy(reply, p.1, co, vec![]).map(|_| ()).map_err(drop))
    }
}
