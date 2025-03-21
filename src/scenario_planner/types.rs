use std::{ffi::OsString, net::SocketAddr};

use http::Uri;

use crate::cli::WebsocatArgs;

#[derive(Debug)]
pub enum Endpoint {
    //@ @inhibit_prefixes
    //@ Connect to a TCP socket by hostname.
    //@ Hostname resolution happens once, on scenario start.
    //@ If multiple address are resolved, they are tried simultaneously, first connected one wins.
    //@
    //@ See prefixes for `TcpConnectByIp`.
    TcpConnectByEarlyHostname {
        varname_for_addrs: String,
    },
    //@ @inhibit_prefixes
    //@ Connect to a TCP socket by hostname.
    //@ Hostname resolution is repeated every time a connection is initiated.
    //@ If multiple address are resolved, they are tried simultaneously, first connected one wins.
    //@
    //@ See prefixes for `TcpConnectByIp`
    /// All TCP connections start as late-resolved when parsing CLI argument,
    /// but may be converted to early-resolved by the patcher.
    TcpConnectByLateHostname {
        hostname: String,
    },
    //@ Connected to a TCP socket using one explicitly specified IPv4 or IPv6 socket address.
    TcpConnectByIp(SocketAddr),
    //@ Listen for incoming TCP connections on one TCP socket, bound to the specified IPv4 or IPv6 address.
    TcpListen(SocketAddr),
    //@ Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process)
    TcpListenFd(i32),
    //@ Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)
    TcpListenFdNamed(String),
    //@ Connect to specified WebSocket plain (insecure) URL
    WsUrl(Uri),
    //@ Connect to specified WebSocket TLS URL
    WssUrl(Uri),
    //@ Listen for incoming WebSocket connections at specified TCP socket address.
    WsListen(SocketAddr),
    //@ Console, terminal: read bytes from stdin, write bytes to stdout.
    //@
    //@ Uses additional thread, which may cause lower latency and throughput.
    Stdio,
    //@ Connect to this UDP socket. Not affected by `--udp-bind-*`` CLI options.
    UdpConnect(SocketAddr),
    //@ Bind UDP socket to this address.
    //@ Command line options greatly affect behaviour of this endpoint. It can be turned into a flexible `UdpConnect` analogue.
    UdpBind(SocketAddr),
    //@ Use inherited pre-bound UDP socket from specified file descriptor.
    UdpFd(i32),
    //@ Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES)
    UdpFdNamed(String),
    //@ Bind UDP socket and spawn a separate task for each client.
    //@ Connections get closed when there are too many active clients or by a timeout.
    UdpServer(SocketAddr),
    //@ Use inherited pre-bound UDP socket from specified file descriptor, spawning a task for each client
    UdpServerFd(i32),
    //@ Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES), spawning a task for each client
    UdpServerFdNamed(String),
    //@ Execute given program as a subprocess and use its stdin/stdout as a socket.
    //@ Specify command line arguments using `--exec-args` command line option.
    Exec(OsString),
    //@ Execute given command line and use its stdin/stdout as a socket.
    Cmd(OsString),
    //@ Byte stream socket that ignores all incoming data and immediately EOF-s read attempts
    DummyStream,
    //@ Datagram socket that ignores all incoming data and signals EOF immediately
    DummyDatagrams,
    //@ Byte stream socket that produces specified content and ignores incoming data
    Literal(String),
    //@ Byte stream socket that produces specified content (base64-encoded) and ignores incoming data
    LiteralBase64(String),

    //@ Connect to the specified UNIX socket path using stream socket
    UnixConnect(OsString),
    //@ Listen specified UNIX socket path for SOCK_STREAM connections
    UnixListen(OsString),
    //@ Connect to the specified abstract-namespaced UNIX socket (Linux)
    AbstractConnect(OsString),
    //@ Listen UNIX socket on specified abstract path (Linux)
    AbstractListen(OsString),

    //@ Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process)
    UnixListenFd(i32),
    //@ Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)
    UnixListenFdNamed(String),

    //@ Use specified inherited file descriptor for reading and writing, assuming it supports `read(2)` and `writev(2)` and can be put in epoll (or analogue).
    //@
    //@ Trying to specify unexisting FD, especially low-numbered (e.g from 3 to 20) may lead to undefined behaviour.
    AsyncFd(i32),

    //@ Connect to specified UNIX SOCK_SEQPACKET socket by path
    //@
    //@ Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace
    SeqpacketConnect(OsString),
    //@ Listen specified UNIX SOCK_SEQPACKET socket
    //@
    //@ Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace
    SeqpacketListen(OsString),
    //@ Connect to specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name
    AbstractSeqpacketConnect(OsString),
    //@ Listen specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name
    AbstractSeqpacketListen(OsString),
    //@ Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process)
    SeqpacketListenFd(i32),
    //@ Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
    //@ with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)
    SeqpacketListenFdNamed(String),
    //@ Byte stream socket for tests that can produce and consume (assert)
    //@ data according to special scenario supplied as an argument
    MockStreamSocket(String),
    //@ Listen for virtual intra-Websocat stream connections at specified address
    RegistryStreamListen(String),
    //@ Connect to a virtual intra-Websocat address using stream socket
    RegistryStreamConnect(String),

    //@ Implementation detail of `reuse-raw:` overlay
    SimpleReuserEndpoint(String, Box<SpecifierStack>),

    //@ Read specified file. Ignores writes.
    ReadFile(OsString),

    //@ Write specified file.
    WriteFile(OsString),

    //@ Append to specified file.
    AppendFile(OsString),

    //@ Generate random bytes
    Random,

    //@ Generate zero bytes
    Zero,

    //@ Implementation detail of `write-splitoff:` overlay
    WriteSplitoff {
        read: Box<SpecifierStack>,
        write: Box<SpecifierStack>,
    },
}

#[derive(Debug)]
pub enum Overlay {
    //@ Send HTTP/1 WebSocket upgrade to specified stream-oriented connection and accept and parse a reply,
    //@ then connects (i.e. exchanges bytes) the downstream connection to upstream.
    //@
    //@ Does not provide WebSocket framing.
    WsUpgrade {
        uri: Uri,
        host: Option<String>,
    },
    //@ Expects a HTTP/1 WebSocket upgrade request from downstream stream socket. If valid, replies with Upgrade HTTP reply.
    //@ After than connects (i.e. exchanges bytes) downstream to upstream.
    //@
    //@ Does not provide WebSocket framing.
    WsAccept {},
    //@ Converts downstream stream to upstream packets using WebSocket framing.
    //@
    //@ Automatically handles WebSocket pings and CloseFrames, but does not fully terminate the connection on CloseFrame, only signaling EOF instead.
    //@
    //@ Client or server mode is chosen depending on prefix you use.
    WsFramer {
        client_mode: bool,
    },
    //@ Combined WebSocket upgrader and framer, but without TCP or TLS things
    //@ URI is taken from --ws-c-uri CLI argument
    //@ If it is not specified, it defaults to `/`, with a missing `host:` header
    WsClient,
    //@ Combined WebSocket acceptor and framer.
    WsServer,
    //@ Establishes client-side TLS connection using specified stream-oriented downstream connection
    TlsClient {
        domain: String,
        varname_for_connector: String,
    },
    //@ Converts downstream stream-oriented socket to packet-oriented socket by chunking the stream arbitrarily (i.e. as syscalls happened to deliver the data)
    //@
    //@ May be automatically inserted in binary (`-b`) mode.
    StreamChunks,
    //@ Convert downstream stream-oriented socket to packet-oriented socket by using newline byte as a packet separator.
    //@ Written data get modified to ensure that one line = one message.
    //@
    //@ May be automatically inserted in text (`-t`) mode.
    LineChunks,
    //@ Convert downstream stream-oriented socket to packet-oriented socket by prefixing each message with its length
    //@ (and maybe other flags, depending on options).
    LengthPrefixedChunks,
    //@ Print encountered data to stderr for debugging
    Log {
        datagram_mode: bool,
    },
    //@ Limit this stream's read buffer size to --read-buffer-limit
    //@ By splitting reads to many (e.g. single byte) chunks, we can
    //@ test and debug trickier code paths in various overlays
    ReadChunkLimiter,
    //@ Limit this stream's write buffer size to --write-buffer-limit
    //@ By enforcing short writes, we can
    //@ test and debug trickier code paths in various overlays
    WriteChunkLimiter,
    //@ Insert write buffering layer that combines multiple write calls to one bigger
    WriteBuffer,
    //@ Share underlying datagram connection between multiple outer users.
    //@
    //@ All users can write messages to the socket, messages would be interleaved
    //@ (though each individual message should be atomic).
    //@ Messages coming from inner socket will be delivered to some one arbitrary connected user.
    //@ If that users disconnect, they will route to some other user.
    //@ A message can be lost when user disconnects.
    //@ User disconnections while writing a message may abort the whole reuser
    //@ (or result in a broken, trimmed message, depending on settings).
    SimpleReuser,

    //@ Only read from inner specifier, route writes to other, CLI-specified Socket
    WriteSplitoff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecifierPosition {
    /// First positional argument of Websocat CLI, for listeners and connectors
    Left,
    /// First positional argument of Websocat CLI, for connectors
    Right,
}

#[derive(Debug)]
pub struct SpecifierStack {
    pub innermost: Endpoint,
    /// zeroth element is the last specified overlay, e.g. `ws-ll:` in `reuse:autoreconnect:ws-ll:tcp:127.0.0.1:1234`.
    pub overlays: Vec<Overlay>,
    pub position: SpecifierPosition,
}

#[derive(Debug)]
pub enum PreparatoryAction {
    ResolveHostname {
        hostname: String,
        varname_for_addrs: String,
    },
    CreateTlsConnector {
        varname_for_connector: String,
    },
    CreateSimpleReuserListener {
        varname_for_reuser: String,
    },
}

pub struct WebsocatInvocationStacks {
    pub left: SpecifierStack,
    pub right: SpecifierStack,
    pub write_splitoff: Option<SpecifierStack>,
}

pub struct WebsocatInvocation {
    pub stacks: WebsocatInvocationStacks,
    pub opts: WebsocatArgs,
    pub beginning: Vec<PreparatoryAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    ByteStream,
    Datarams,
}

pub use super::scenarioprinter::ScenarioPrinter;
pub use super::utils::IdentifierGenerator;
pub struct ScenarioPrintingEnvironment<'a> {
    pub printer: &'a mut ScenarioPrinter,
    pub opts: &'a WebsocatArgs,
    pub vars: &'a mut IdentifierGenerator,
    pub position: SpecifierPosition,
}
