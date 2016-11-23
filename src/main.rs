#![recursion_limit = "1024"] // error_chain
 
extern crate websocket;
extern crate env_logger;
#[macro_use]
extern crate log;
#[macro_use]
extern crate error_chain;
extern crate url;
extern crate clap;

use std::thread;
use std::io::{stdin,stdout};

use websocket::{Message, Sender, Receiver, DataFrame};
use websocket::message::Type;
use websocket::client::request::Url;
use websocket::Client;

use std::borrow::Borrow;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Write, Read};

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Log(log::SetLoggerError);
        Url(::url::ParseError);
        Ws(::websocket::result::WebSocketError);
        VarError(::std::env::VarError);
        RE(std::sync::mpsc::RecvError);
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
    
        let receive_loop = thread::spawn(move || -> Result<()> {
            // Actual data transfer happens here
            ::std::io::copy(&mut reader1, &mut writer2)?;
            writer2.write(b"")?; // signal close
            Ok(())
        });
    
        // Actual data transfer happens here
        ::std::io::copy(&mut reader2, &mut writer1)?;
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

fn get_stdio_peer() -> Result<Peer<std::io::Stdin, std::io::Stdout>> {
    Ok(
        Peer {
            reader : stdin(),
            writer : stdout(),
        }
    )
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

fn get_peer_by_spec(specifier: &str, server: bool) -> Result<IPeer> {
    let _ = server;
    match specifier {
        x if x == "-"               => Ok(get_stdio_peer()?.upcast()),
        x if x.starts_with("ws:")   => Ok(get_websocket_peer(x)?.upcast()),
        x if x.starts_with("wss:")  => Ok(get_websocket_peer(x)?.upcast()),
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
        .about("Exchange binary data between websocket and something.\nSocat analogue with websockets.")
        .arg(::clap::Arg::with_name("listener_spec")
             .help("Listener specifier.")
             .required(true)
             .index(1))
        .arg(::clap::Arg::with_name("connector_spec")
             .help("Connector specifier.")
             .required(true)
             .index(2))
        .after_help(r#"
Specifiers are:
  ws[s]://<rest of websocket URL>    websockets
  -                                  stdin/stdout
  (more to be implemented)
  
Examples:
  websocat - wss://myserver/mysocket
    Connect stdin/stdout to secure web socket once.
    Currently it is the only working example.
"#)
        .get_matches();

    let listener_spec  = matches.value_of("listener_spec") .ok_or("no listener_spec" )?;
    let connector_spec = matches.value_of("connector_spec").ok_or("no connector_spec")?;
    
    let des = DataExchangeSession {
        peer1 : get_peer_by_spec(listener_spec,  true )?,
        peer2 : get_peer_by_spec(connector_spec, false)?,
    };
    
    des.data_exchange()?;

    debug!("Exited");
    Ok(())
}

fn main() {
    if let Err(x) = try_main() {
        let _ = writeln!(::std::io::stderr(), "{:?}", x);
    }
}

