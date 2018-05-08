extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

extern crate env_logger;

#[macro_use]
extern crate structopt;

use structopt::StructOpt;

use tokio_core::reactor::{Core};
use websocat::{spec, WebsocatConfiguration, Options};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(StructOpt, Debug)]
#[structopt(after_help = "
Basic examples:
  Connect stdin/stdout to a websocket:
    websocat - ws://echo.websocket.org/
    
  Listen websocket and redirect it to a TCP port:
    websocat ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    
  See more examples with the --long-help option
  
Short list of specifiers (see --long-help):
  ws:// wss:// - inetd: ws-listen: inetd-ws: tcp: tcp-l: ws-c:
  autoreconnect: reuse: mirror: threadedstdio: clogged:
  literal: literalreply: assert: udp-connect: open-async:
  readfile: writefile: open-fd:
")]
struct Opt {
    /// First, listening/connecting specifier. See --long-help for info about specifiers.
    s1: String,
    /// Second, connecting specifier
    s2: String,
    
    #[structopt(short = "u", long = "unidirectional", help="Inhibit copying data from right specifier to left")]
    unidirectional: bool,
    #[structopt(short = "U", long = "unidirectional-reverse", help="Inhibit copying data from left specifier to right")]
    unidirectional_reverse: bool,
    
    #[structopt(short = "t", long = "text", help="Send text WebSocket messages instead of binary")]
    websocket_text_mode: bool,
    
    #[structopt(long="oneshot", help="Serve only once")]
    oneshot: bool,
    
    #[structopt(long="long-help", help="Show full help aboput specifiers and examples")]
    longhelp: bool,
    
    #[structopt(long="dump-spec", help="Instead of running, dump the specifiers representation to stdout")]
    dumpspec: bool,
    
    #[structopt(long="protocol", help="Specify Sec-WebSocket-Protocol: header")]
    websocket_protocol: Option<String>,
    
    #[structopt(long="udp-oneshot", help="udp-listen: replies only one packet per client")]
    udp_oneshot_mode: bool,
}

fn longhelp() {
    println!("(see also usual --help message)
    
Full list of specifiers:
  `-` -- Stdin/stdout
    Read input from console, print to console.
    Can be specified only one time.
    Aliases: `stdio:`, `inetd:`
    
    `inetd:` also disables logging to stderr.
    
    Example: like `cat(1)`.
      websocat - -
      
    Example: for inetd mode
      websocat inetd: literal:$'Hello, world.\n'
    
  `ws://<url>`, `wss://<url>` -- WebSocket client
    Example: forward port 4554 to a websocket
      websocat tcp-l:127.0.0.1:4554 wss://127.0.0.1/some_websocket
      
  `ws-listen:<spec>` - Listen for websocket connections
    A combining specifier, but given IPv4 address as argument auto-inserts `tcp-l:`
    Aliases: `listen-ws:` `ws-l:` `l-ws:`
    
    Example:
        websocat ws-l:127.0.0.1:8808 -
    
    Example: the same, but more verbose:
        websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-
  
  `inetd-ws:` - Alias of `ws-l:inetd:`
  
    Example of inetd.conf line:
      1234 stream tcp nowait myuser  /opt/websocat websocat inetd-ws: tcp:127.0.0.1:22

  
  `tcp:<hostport>` - connect to specified TCP host and port
    Aliases: `tcp-connect:`,`connect-tcp:`,`c-tcp:`,`tcp-c:`
    
    Example: like netcat
      websocat - tcp:127.0.0.1:22
      
    Example: IPv6
      websocat - tcp:[::1]:22
    
  `tcp-l:<hostport>` - listen TCP port on specified address
    Aliases: `l-tcp:`  `tcp-listen:` `listen-tcp:`
    
    Example: echo server
      websocat tcp-l:0.0.0.0:1441 mirror:
      
  `udp:<hostport>` - send and receive packets to specified UDP socket
    Aliases: `udp-connect:` `connect-udp:` `c-udp:` `udp-c:`
    
  `udp-listen:<hostport>` - bind to socket on host and port
    Aliasses: `udp-l:`, `l-udp:`, `listen-udp:`
    
    Note that it is not a multiconnect specifier: entire lifecycle
    of the UDP socket is the same connection.
    
    Packets get sent to the most recent seen peer.
    If no peers are seen yet, it waits for the first packet.
    
    File a feature request on Github if you want proper DNS-like request-reply UDP mode here.
  
  `ws-connect:<spec>` - low-level WebSocket connector
    A combining specifier. Underlying specifier is should be after the colon.
    URL and Host: header being sent are independent from underlying specifier
    Aliases: `ws-c:` `c-ws:` `connect-ws:`
    
    Example:
      websocat - ws-c:tcp:127.0.0.1:8808
  
  `autoreconnect:<spec>` - Auto-reconnector
    Re-establish underlying specifier on any error or EOF
    
    Example: keep connecting to the port or spin 100% CPU trying if it is closed.
      websocat - autoreconnect:tcp:127.0.0.1:5445
      
    TODO: implement timeouts
    
  `reuse:<spec>` - Reuse one connection for serving multiple clients
    Better suited for unidirectional connections
    
    Example (unreliable): don't disconnect SSH when websocket reconnects
      websocat ws-l:[::]:8088 reuse:tcp:127.0.0.1:22

  `threadedstdio:` - Stdin/stdout, spawning a thread
    Like `-`, but forces threaded mode instead of async mode
    Use when standard input is not `epoll(7)`-able.
    Replaces `-` when `no_unix_stdio` Cargo feature is activated
  
  `mirror:` - Simply copy output to input
  
  `open-async:<path>` - Open file for read and write and use it like a socket
    Not for regular files, see readfile: and writefile: instead.
  
    Example:
      websocat - open-async:/dev/null
      
  `open-fd:<number>` - Use specified file descriptor like a socket

  `readfile:<path>` - synchronously read files
    Blocking on operations with the file pauses the whole process
    
    Example:
      websocat ws-l:127.0.0.1:8000 readfile:hello.json
      
  `write:<path>` - synchronously write files
    Blocking on operations with the file pauses the whole process
    Files are opened in overwrite mode.
    
    Example:
      websocat ws-l:127.0.0.1:8000 reuse:writefile:log.txt
  
  `clogged:` - Do nothing
    Don't read or write any bytes. Keep connections hanging.
    
  `literal:<string>` - Output a string, discard input.
    Ignore all input, use specified string as output.
  
  `literalreply:<string>` - Reply with this string for each input packet
  
  `assert:<string>` - Check the input.
    Read entire input and panic the program if the input is not equal
    to the specified string.
    
  TODO:
  `exec:`, `unix-l:`
  
More examples:
  Wacky mode:
    websocat ws-l:ws-l:ws-c:- tcp:127.0.0.1:5678
    
    Connect to a websocket using stdin/stdout as a transport,
    then accept a websocket connection over this previous websocket as a transport,
    then connect to a websocket using previous step as a transport,
    then forward resulting connection to the TCP port.
    
    (Excercise to the reader: manage to actually connect to it).
");
}

fn run() -> Result<()> {
    if std::env::args().nth(1).unwrap_or_default() == "--long-help" {
        longhelp();
        return Ok(());
    }

    let cmd = Opt::from_args();
    
    if cmd.longhelp {
        longhelp();
        return Ok(());
    }
    
    if false 
        || cmd.oneshot
    {
        Err("This mode is not implemented")?
    }
    
    let opts = {
        macro_rules! opts {
            ($($o:ident)*) => {
                Options {
                    $($o : cmd.$o,)*
                }
            }
        }
        opts!(
            websocket_text_mode
            websocket_protocol
            udp_oneshot_mode
            unidirectional
            unidirectional_reverse
        )
    };
    
    let s1 = spec(&cmd.s1)?;
    let s2 = spec(&cmd.s2)?;
    
    let mut websocat = WebsocatConfiguration { opts, s1, s2 };
    
    while let Some(concern) = websocat.get_concern() {
        use websocat::ConfigurationConcern::*;
        if concern == StdinToStdout {
            if cmd.dumpspec {
                println!("cat mode");
                return Ok(())
            }
            
            // Degenerate mode: just copy stdin to stdout and call it a day
            ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
            return Ok(())
        }
        
        if concern == DegenerateMode {
            if cmd.dumpspec {
                println!("noop");
            }
            return Ok(())
        }
        
        if concern == StdioConflict {
            Err("Too many usages of stdin/stdout")?;
        }
        
        if concern == NeedsStdioReuser {
            //Err("Stdin/stdout is used without a `reuse:` overlay.")?;
            eprintln!("Warning: replies on stdio get directed at random connected client");
            websocat = websocat.auto_install_reuser();
            continue;
        }
        
        if concern == MultipleReusers {
            eprintln!("Specifier dump: {:?} {:?}", websocat.s1, websocat.s2);
            Err("Multiple reusers is not allowed")?;
        }
        break;
    }
    
    if cmd.dumpspec {
        println!("{:?}", websocat.s1);
        println!("{:?}", websocat.s2);
        println!("{:?}", websocat.opts);
        return Ok(())
    }

    let mut core = Core::new()?;

    let prog = websocat.serve(core.handle(), std::rc::Rc::new(|e| {
        eprintln!("websocat: {}", e);
    }));
    core.run(prog).map_err(|()|"error running".to_string())?;
    Ok(())
}

fn main() {
    env_logger::init();
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
