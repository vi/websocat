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
const QSIZ : usize = 1;

use std::thread;
use std::sync::mpsc::sync_channel;
use std::io::{stdin,stdout};

use websocket::{Message, Sender, Receiver};
use websocket::message::Type;
use websocket::client::request::Url;
use websocket::Client;

use std::io::{Read,Error as IoError,ErrorKind as IoErrorKind,Write};

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
        if buf.len() > 0 {
            ret = self.0.send_message(&message);
        } else {
            // Interpret zero length buffer is request
            // to close communication
            ret = self.0.send_message(&Message::close());
        }
        ret.map_err(|e|IoError::new(IoErrorKind::BrokenPipe, e))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> ::std::io::Result<()> {
        Ok(())
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

    let (mut sender, mut receiver) = response.begin().split();

    //let (tx, rx) = sync_channel(QSIZ);

    //let tx_1 = tx.clone();

    /*let send_loop = thread::spawn(move || {
        if let Err(er) = (|| -> Result<()> {
            loop {
                // Send loop
                let message: Message = rx.recv()?;
                if message.opcode == Type::Close { 
                    break; 
                }
                sender.send_message(&message)?;
            }
            Ok(())
        })() {
            error!("Receive loop: {}", er);
        }
        let _ = sender.send_message(&Message::close());
    });*/

    let receive_loop = thread::spawn(move || -> Result<()> {
        if let Err(x) = (|| -> Result<()> {
            for m in receiver.incoming_messages() {
                let message : websocket::Message = try!(m);
                match message.opcode {
                    Type::Close => {
                        return Ok(());
                    }
                    Type::Ping => {
                        /*tx_1.send(
                            Message::pong(
                                message.payload
                            )).map_err(|_|"Failed pong")?*/
                    }
                    // Say what we received
                    _ => {
                        use std::borrow::Borrow;
                        use std::io::Write;
                        let msgpayload : &[u8] = message.payload.borrow();
                        debug!("Received message of {} bytes", msgpayload.len());
                        stdout().write_all(msgpayload)?;
                        stdout().flush()?;
                    }
                }
            }
            Ok(())
        })() {
            error!("Error on receive loop: {}", x);
        }
        //let _ = tx_1.send(Message::close());
        Ok(())
    });

    //let mut buffer : [u8; BUFSIZ] = [0; BUFSIZ];
    
    /*loop {
        let data : Vec<u8> = match stdin().read(&mut buffer) {
            Ok(0) => break,
            Ok(ret) => {
                debug!("Sending {} bytes of data", ret);
                buffer[0..ret].to_vec()
            }
            Err(ref e) if e.kind() == IoErrorKind::Interrupted => continue,
            Err(ref e) if e.kind() == IoErrorKind::WouldBlock  => continue,
            Err(e) => {
                error!("Read error: {}", e);
                break;
            }
        };

        let message = Message::binary(data);

        match tx.send(message) {
            Ok(()) => (),
            Err(e) => {
                error!("Main Loop: {:?}", e);
                break;
            }
        }
    }
    let _ = tx.send(Message::close());*/

    // We're exiting
    
    let mut wr = SenderWrapper(sender);
    
    // Actual data transfer happens here
    ::std::io::copy(&mut stdin(), &mut wr)?;
    
    wr.write(b"")?; // Signal close

    info!("Waiting for child threads to exit");

    //let _ = send_loop.join();
    let _ = receive_loop.join();

    info!("Exited");
    Ok(())
}

fn main() {
    if let Err(x) = try_main() {
        let _ = writeln!(::std::io::stderr(), "{:?}", x);
    }
}

