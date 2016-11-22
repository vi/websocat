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
                
                buf[0..len].clone_from_slice(msgpayload);
                
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
    
        info!("Waiting for child threads to exit");
    
        receive_loop.join().map_err(|x|format!("{:?}",x))?
    }
}

fn try_main() -> Result<()> {
    //env_logger::init()?;
    init_logger()?;

    // setup command line arguments
    let matches = ::clap::App::new("WS Command Line Client")
        .version("0.1")
        .author("Vitaly \"_Vi\" Shukela <vi0oss@gmail.com>")
        .about("Send binary data from stdin to a WebSocket and back to stdout.")
        .arg(::clap::Arg::with_name("URL")
             .help("The URL of the WebSocket server.")
             .required(true)
             .index(1)).get_matches();


    let url = Url::parse(matches.value_of("URL").ok_or("no URL")?)?;

    info!("Connecting to {}", url);

    let request = Client::connect(url)?;

    let response = request.send()?; // Send the request and retrieve a response

    info!("Validating response...");

    response.validate()?; // Validate the response

    info!("Successfully connected");

    let (sender, receiver) = response.begin().split();
    
    let des = DataExchangeSession {
        peer1 : Peer {
            reader : ReceiverWrapper(receiver),
            writer : SenderWrapper(sender),
        },
        peer2 : Peer {
            reader : stdin(),
            writer : stdout(),
        },
    };
    
    des.data_exchange()?;

    info!("Exited");
    Ok(())
}

fn main() {
    if let Err(x) = try_main() {
        let _ = writeln!(::std::io::stderr(), "{:?}", x);
    }
}

