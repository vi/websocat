use super::*;

pub type HttpRequestWithResponseSlot = (
    http::Request<hyper::Body>,
    tokio::sync::oneshot::Sender<http::Response<hyper::Body>>,
);
pub type ByteStreamSource = Pin<Box<dyn AsyncRead + Send + 'static>>;
pub type DatagramSource =
    Pin<Box<dyn futures::stream::Stream<Item = Result<bytes::Bytes>> + Send + 'static>>;
pub type HttpSource = Pin<
    Box<dyn futures::stream::Stream<Item = Result<HttpRequestWithResponseSlot>> + Send + 'static>,
>;
pub type ByteStreamSink = Pin<Box<dyn AsyncWrite + Send + 'static>>;
pub type DatagramSink =
    Pin<Box<dyn futures::sink::Sink<bytes::Bytes, Error = anyhow::Error> + Send + 'static>>;
pub type HttpSink = Pin<
    Box<
        dyn futures::sink::Sink<HttpRequestWithResponseSlot, Error = anyhow::Error>
            + Send
            + 'static,
    >,
>;
pub type ClosingNotification = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub enum Source {
    ByteStream(ByteStreamSource),
    Datagrams(DatagramSource),
    Http(HttpSource),
    None,
}

pub enum Sink {
    ByteStream(ByteStreamSink),
    Datagrams(DatagramSink),
    Http(HttpSink),
    None,
}

/// A bi-directional channel + special closing notification
pub struct Bipipe {
    pub r: Source,
    pub w: Sink,
    pub closing_notification: Option<ClosingNotification>,
}

impl std::fmt::Debug for Bipipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.r {
            Source::ByteStream(..) => write!(f, "(r=ByteStream")?,
            Source::Datagrams(..) => write!(f, "(r=Datagrams")?,
            Source::Http(..) => write!(f, "(r=Http")?,
            Source::None => write!(f, "(r=None")?,
        };
        match self.w {
            Sink::ByteStream(..) => write!(f, " w=ByteStream")?,
            Sink::Datagrams(..) => write!(f, " w=Datagrams")?,
            Sink::Http(..) => write!(f, " w=Http")?,
            Sink::None => write!(f, " w=None")?,
        };
        if self.closing_notification.is_some() {
            write!(f, " +CN)")?;
        } else {
            write!(f, ")")?;
        }
        Ok(())
    }
}


#[derive(Clone)]
pub struct RunContext {
    /// for starting running child nodes before this one
    pub nodes: Arc<Tree>,

    /// Mutually exclusive with `left_to_right_things_to_read_from`
    /// Used "on the left (server) sise" of websocat call to fill in various
    /// incoming connection parameters like IP address or requesting URL.
    ///
    /// Hashmap keys are arbitrary identifiers - various nodes need to aggree in them
    pub left_to_right_things_to_be_filled_in: Option<Arc<Mutex<Properties>>>,

    /// Mutually exclusive with `left_to_right_things_to_be_filled_in`
    /// Use d "on the right side" of websocat call to act based on properties
    /// collected during acceping incoming connection
    pub left_to_right_things_to_read_from: Option<Arc<Mutex<Properties>>>,
}

static_assertions::assert_impl_all!(RunContext: Send);

/// Opaque object that can be used as a storage space for individual nodes
pub type AnyObject = Box<dyn std::any::Any + Send + 'static>;

/// Used to support serving multiple clients, allowing to restart Websocat session from
/// nodes like "tcp-listen", passing listening sockets though `AnyObject`.
///
/// First time `you_are_called_not_the_first_time` is None, meaning that e.g. `TcpListener` should be
/// created from scratch.
///
/// Invoking `call_me_again_with_this` spawns a Tokio task that should ultimately return back
/// to the node that issued `call_me_again_with_this`, but with `you_are_called_not_the_first_time`
/// filled in, so `TcpListener` (with potential next pending connection) should be restored
/// from the `AnyObject` instead of being created from stratch.
pub struct ServerModeContext {
    pub you_are_called_not_the_first_time: Option<AnyObject>,

    #[allow(clippy::unused_unit)]
    pub call_me_again_with_this: Box<dyn FnOnce(AnyObject) -> () + Send + 'static>,
}
