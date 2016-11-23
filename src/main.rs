#![recursion_limit = "1024"] // error_chain
 
extern crate websocket;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
extern crate url;
extern crate clap;

const BUFSIZ : usize = 8192;

use std::thread;
use std::io::{stdin,stdout};

use websocket::{Message, Sender, Receiver, DataFrame, Server as WsServer};
use websocket::message::Type;
use websocket::client::request::Url;
use websocket::Client;

use std::borrow::Borrow;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Write, Read};

error_chain! {
    foreign_links {
        ::std::io::Error, Io;
        log::SetLoggerError, Log;
        ::url::ParseError, Url;
        ::websocket::result::WebSocketError, Ws;
        ::std::env::VarError, Ev;
    }
    errors {
        InvalidSpecifier(t : String) {
            description("invalid specifier")
            display("Invalid client or server specifier `{}`", t)
        }
    }
}

// Initialize logger with default "info" log level:
fn init_logger() -> Result<()> {
    let mut builder = env_logger::LogBuilder::new();
    builder.filter(None, log::LogLevelFilter::Info);
    if ::std::env::var("RUST_LOG").is_ok() {
       builder.parse(&::std::env::var("RUST_LOG")?);
    }
    builder.init()?;
    Ok(())
}

struct SenderWrapper<T: Sender> (T);

impl<T: Sender> ::std::io::Write for SenderWrapper<T> {
    fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
        let message = Message::binary(buf);
        let ret;
        let len = buf.len();
        if len > 0 {
            debug!("Sending message of {} bytes", len);
            ret = self.0.send_message(&message);
        } else {
            // Interpret zero length buffer is request
            // to close communication
            
            debug!("Sending the closing message");
            ret = self.0.send_message(&Message::close());
        }
        ret.map_err(|e|IoError::new(IoErrorKind::BrokenPipe, e))?;
        Ok(len)
    }
    fn flush(&mut self) -> ::std::io::Result<()> {
        Ok(())
    }
}

struct ReceiverWrapper<T: Receiver<DataFrame>> (T);

impl<T:Receiver<DataFrame>> ::std::io::Read for ReceiverWrapper<T> {
    fn read(&mut self, buf: &mut [u8]) -> ::std::io::Result<usize> {
        let ret = self.0.recv_message();
        let msg : Message = ret.map_err(|e|IoError::new(IoErrorKind::BrokenPipe, e))?;
        
        match msg.opcode {
            Type::Close => {
                Ok(0)
            }
            Type::Ping => {
                // Sender used to be in a separate thread with a channel
                // now there's no channel, so trickier to combine ping replies
                // and usual data exchange
                unimplemented!();
            }
            _ => {
                let msgpayload : &[u8] = msg.payload.borrow();
                let len = msgpayload.len();
                debug!("Received message of {} bytes", len);
                
                assert!(buf.len() >= len);
                
                buf[0..len].copy_from_slice(msgpayload);
                
                Ok(len)
            }
        }
    }
}

struct Peer<R, W>
    where R : Read + Send + 'static, W: Write + Send + 'static
{
    reader: R,
    writer: W,
}

type IPeer = Peer<Box<Read+Send>, Box<Write+Send>>;

struct DataExchangeSession<R1, R2, W1, W2> 
    where R1 : Read  + Send + 'static, 
          R2 : Read  + Send + 'static,
          W1 : Write + Send + 'static,
          W2 : Write + Send + 'static,
{
    peer1: Peer<R1, W1>,
    peer2: Peer<R2, W2>,
}

// Derived from https://doc.rust-lang.org/src/std/up/src/libstd/io/util.rs.html#46-61
pub fn copy_with_flushes<R: ?Sized, W: ?Sized>(reader: &mut R, writer: &mut W) -> ::std::io::Result<u64>
    where R: Read, W: Write
{
    let mut buf = [0; BUFSIZ];
    let mut written = 0;
    loop {
        let len = match reader.read(&mut buf) {
            Ok(0) => return Ok(written),
            Ok(len) => len,
            Err(ref e) if e.kind() == IoErrorKind::Interrupted => continue,
            Err(ref e) if e.kind() == IoErrorKind::WouldBlock => continue,
            Err(e) => return Err(e),
        };
        writer.write_all(&buf[..len])?;
        writer.flush()?;
        written += len as u64;
    }
}

impl<R1,R2,W1,W2> DataExchangeSession<R1,R2,W1,W2> 
    where R1 : Read  + Send + 'static,
          R2 : Read  + Send + 'static, 
          W1 : Write + Send + 'static,
          W2 : Write + Send + 'static,
{
    fn data_exchange(self) -> Result<()> {
    
        let mut reader1 = self.peer1.reader;
        let mut writer1 = self.peer1.writer;
        let mut reader2 = self.peer2.reader;
        let mut writer2 = self.peer2.writer;
    
        let receive_loop = thread::Builder::new().spawn(move || -> Result<()> {
            // Actual data transfer happens here
            copy_with_flushes(&mut reader1, &mut writer2)?;
            writer2.write(b"")?; // signal close
            Ok(())
        })?;
    
        // Actual data transfer happens here
        copy_with_flushes(&mut reader2, &mut writer1)?;
        writer1.write(b"")?; // Signal close
    
        debug!("Waiting for receiver side to exit");
    
        receive_loop.join().map_err(|x|format!("{:?}",x))?
    }
}

fn get_websocket_peer(urlstr: &str) -> Result<
        Peer<
            ReceiverWrapper<websocket::client::Receiver<websocket::WebSocketStream>>,
            SenderWrapper<websocket::client::Sender<websocket::WebSocketStream>>>
        > {
    let url = Url::parse(urlstr)?;

    info!("Connecting to {}", url);

    let request = Client::connect(url)?;

    let response = request.send()?; // Send the request and retrieve a response

    info!("Validating response...");

    response.validate()?; // Validate the response

    info!("Successfully connected");

    let (sender, receiver) = response.begin().split();
    
    let peer = Peer {
        reader : ReceiverWrapper(receiver),
        writer : SenderWrapper(sender),
    };
    Ok(peer)
}

fn get_tcp_peer(addr: &str) -> Result<
        Peer<
            ::std::net::TcpStream,
            ::std::net::TcpStream,
        >> {
    let sock = ::std::net::TcpStream::connect(addr)?;

    let peer = Peer {
        reader : sock.try_clone()?,
        writer : sock.try_clone()?,
    };
    info!("Connected to TCP {}", addr);
    Ok(peer)
}

fn get_stdio_peer() -> Result<Peer<std::io::Stdin, std::io::Stdout>> {
    Ok(
        Peer {
            reader : stdin(),
            writer : stdout(),
        }
    )
}




struct TcpServer(::std::net::TcpListener);

impl TcpServer {
    fn new(addr: &str) -> Result<Self> {
        Ok(TcpServer(::std::net::TcpListener::bind(addr)?))
    }
}

impl Server for TcpServer {    
    fn accept_client(&mut self) -> Result<IPeer> {
        let (sock, addr) = self.0.accept()?;
        info!("TCP client connection from {}", addr);
        let peer = Peer {
            reader : sock.try_clone()?,
            writer : sock.try_clone()?,
        };
        Ok(peer.upcast())
    }
}




struct WebsockServer<'a>(WsServer<'a>);

impl<'a> WebsockServer<'a> {
    fn new(addr: &str) -> Result<Self> {
        Ok(WebsockServer(WsServer::bind(addr)?))
    }
}

impl<'a> Server for WebsockServer<'a> {    
    fn accept_client(&mut self) -> Result<IPeer> {
        let connection = self.0.accept()?;
        info!("WebSocket client connection ...");
        let request = connection.read_request()?;
        request.validate()?;
        let response = request.accept(); // Form a response
        let mut client = response.send()?; // Send the response

        let ip = client.get_mut_sender()
            .get_mut()
            .peer_addr()
            .unwrap();

        info!("... from IP {}", ip);

        let (sender, receiver) = client.split();

        let peer = Peer {
            reader : ReceiverWrapper(receiver),
            writer : SenderWrapper(sender),
        };
        Ok(peer.upcast())
    }
}






impl<R,W> Peer<R,W> 
    where R : Read + Send + 'static, W: Write + Send + 'static
{
    fn upcast(self) -> IPeer  {
        Peer {
            reader: Box::new(self.reader) as Box<Read +Send>,
            writer: Box::new(self.writer) as Box<Write+Send>,
        }
    }
}


trait Server
{
    fn accept_client(&mut self) -> Result<IPeer>;
    
    fn start_serving(&mut self, spec2: &str, once: bool) -> Result<()> {
        let spec2s = spec2.to_string();
        let closure = move |peer, spec2 : String|{
            let spec2_ = get_peer_by_spec(spec2.as_str())?;
            let peer2 = match spec2_ {
                Spec::Server(mut x) => {
                    x.accept_client()?
                }
                Spec::Client(p1) => {
                    p1
                }
            };
            let des = DataExchangeSession {
                peer1 : peer,
                peer2 : peer2,
            };
            
            des.data_exchange()
        };
        if once {
            let peer = self.accept_client()?;
            closure(peer, spec2s)
        } else {
            let cl2 = ::std::sync::Arc::new(closure);
            loop {
                let ret = self.accept_client();
                let peer = match ret {
                    Ok(x) => x,
                    Err(er) => {
                        warn!("Can't accept client: {}", er);
                        continue;
                    }
                };
                let cl3 = cl2.clone();
                let spec2s2 = spec2s.clone();
                if let Err(x) = thread::Builder::new().spawn(move|| {
                    if let Err(x) = cl3(peer, spec2s2) {
                        warn!("Error while serving: {}", x);
                    }
                }) {
                    warn!("Error creating thread: {}", x);
                    thread::sleep(::std::time::Duration::from_millis(200));
                }
            }
        }
    }
    
    fn upcast(self) -> Box<Server+Send> 
        where Self : Sized + Send + 'static
        { Box::new(self) as Box<Server+Send> }
}

fn main2(spec1: &str, spec2: &str, once: bool) -> Result<()> {
    let spec1_ = get_peer_by_spec(spec1)?;
    
    match spec1_ {
        Spec::Server(mut x) => {
            x.start_serving(spec2, once)
        }
        Spec::Client(p1) => {
            let spec2_ = get_peer_by_spec(spec2)?;
            
            let otherpeer = match spec2_ {
                Spec::Server(mut x) => {
                    let t = x.accept_client()?;
                    t
                }
                Spec::Client(p2) => {
                    p2
                }
            };
            
            let des = DataExchangeSession {
                peer1 : p1,
                peer2 : otherpeer,
            };
            
            des.data_exchange()
        }
    }
}

enum Spec {
    Server(Box<Server + Send>),
    Client(IPeer)
}

fn get_peer_by_spec(specifier: &str) -> Result<Spec> {
    use Spec::{Server,Client};
    match specifier {
        x if x == "-"               => Ok(Client(get_stdio_peer()?.upcast())),
        x if x.starts_with("ws:")   => Ok(Client(get_websocket_peer(x)?.upcast())),
        x if x.starts_with("wss:")  => Ok(Client(get_websocket_peer(x)?.upcast())),
        x if x.starts_with("tcp:")  => Ok(Client(get_tcp_peer(&x[4..])?.upcast())),
        x if x.starts_with("l-tcp:")  => Ok(Server(TcpServer::new(&x[6..])?.upcast())),
        x if x.starts_with("l-ws:")  => Ok(Server(WebsockServer::new(&x[5..])?.upcast())),
        x => Err(ErrorKind::InvalidSpecifier(x.to_string()).into()),
    }
}

fn try_main() -> Result<()> {
    //env_logger::init()?;
    init_logger()?;

    // setup command line arguments
    let matches = ::clap::App::new("websocat")
        .version("0.1")
        .author("Vitaly \"_Vi\" Shukela <vi0oss@gmail.com>")
        .about("Exchange binary data between binary websocket and something.\nSocat analogue with websockets.")
        .arg(::clap::Arg::with_name("spec1")
             .help("First specifier.")
             .required(true)
             .index(1))
        .arg(::clap::Arg::with_name("spec2")
             .help("Second specifier.")
             .required(true)
             .index(2))
        .after_help(r#"
Specifiers can be:
  ws[s]://<rest of websocket URL>   Connect to websocket
  l-ws:host:port                    Listen unencrypted websocket
  tcp:host:port                     Connect to TCP
  l-tcp:host:port                   Listen TCP connections
  -                                 stdin/stdout
  (more to be implemented)
  
Examples:
  websocat l-tcp:0.0.0.0:9559 ws://echo.websocket.org/
    Listen port 9959 on address :: and forward 
    all connections to a public loopback websocket
  websocat l-ws:127.0.0.1:7878 tcp:127.0.0.1:1194
    Listen websocket and forward connections to local tcp
    Use nginx proxy for SSL if you want
  websocat - wss://myserver/mysocket
    Connect stdin/stdout to a secure web socket.
    Like netcat, but for websocket.
  websocat ws://localhost:1234/ tcp:localhost:1235
    Connect both to websocket and to TCP and exchange data.
    
Specify listening part first, unless you want websocat to serve once.

IPv6 supported, just use specs like `l-ws:::1:4567`

Web socket usage is not obligatory, you any specs on both sides.
"#)
        .get_matches();

    let spec1  = matches.value_of("spec1") .ok_or("no listener_spec" )?;
    let spec2 = matches.value_of("spec2").ok_or("no connector_spec")?;
    
    main2(spec1, spec2, false)?;

    debug!("Exited");
    Ok(())
}

fn main() {
    if let Err(x) = try_main() {
        let _ = writeln!(::std::io::stderr(), "{:?}", x);
    }
}

