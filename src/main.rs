#[macro_use]
extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

extern crate env_logger;

#[cfg(feature="openssl-probe")]
extern crate openssl_probe;

#[macro_use]
extern crate structopt;

use structopt::StructOpt;

use tokio_core::reactor::Core;

use websocat::{spec, Options, SpecifierClass, WebsocatConfiguration};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(StructOpt, Debug)]
#[structopt(
    after_help = "
Basic examples:
  Command-line websocket client:
    websocat ws://echo.websocket.org/
    
  Listen websocket and redirect it to a TCP port:
    websocat ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    
  See more examples with the --long-help option
  
Short list of specifiers (see --long-help):
  ws:// wss:// - inetd: ws-listen: inetd-ws: tcp: tcp-l: ws-c:
  autoreconnect: reuse: mirror: threadedstdio: clogged:
  literal: literalreply: assert: udp-connect: open-async:
  readfile: writefile: open-fd: unix-connect: unix-listen:
  unix-dgram: abstract-connect: abstract-listen:
  exec: sh-c:
", usage="websocat [FLAGS] [OPTIONS] <s1> [s2]"
)]
struct Opt {
    /// First, listening/connecting specifier. See --long-help for info about specifiers.
    s1: String,
    /// Second, connecting specifier
    s2: Option<String>,

    #[structopt(
        short = "u",
        long = "unidirectional",
        help = "Inhibit copying data from right specifier to left"
    )]
    unidirectional: bool,
    #[structopt(
        short = "U",
        long = "unidirectional-reverse",
        help = "Inhibit copying data from left specifier to right"
    )]
    unidirectional_reverse: bool,

    #[structopt(
        long = "exit-on-eof",
        short = "E",
        help = "Close a data transfer direction if the other one reached EOF"
    )]
    exit_on_eof: bool,

    #[structopt(
        short = "t", long = "text", help = "Send text WebSocket messages instead of binary"
    )]
    websocket_text_mode: bool,

    #[structopt(long = "oneshot", help = "Serve only once. Not to be confused with -1 (--one-message)")]
    oneshot: bool,

    #[structopt(long = "long-help", help = "Show full help aboput specifiers and examples")]
    longhelp: bool,

    #[structopt(
        long = "dump-spec",
        help = "Instead of running, dump the specifiers representation to stdout"
    )]
    dumpspec: bool,

    #[structopt(long = "protocol", help = "Specify Sec-WebSocket-Protocol: header")]
    websocket_protocol: Option<String>,

    #[structopt(long = "udp-oneshot", help = "udp-listen: replies only one packet per client")]
    udp_oneshot_mode: bool,

    #[structopt(long = "unlink", help = "Unlink listening UNIX socket before binding to it")]
    unlink_unix_socket: bool,

    #[structopt(
        long = "exec-args",
        raw(allow_hyphen_values = r#"true"#),
        help = "Arguments for the `exec:` specifier. Must be the last option, everything after it gets into the exec args list."
    )]
    exec_args: Vec<String>,

    #[structopt(
        long = "ws-c-uri", help = "URI to use for ws-c: specifier", default_value = "ws://0.0.0.0/"
    )]
    ws_c_uri: String,

    #[structopt(
        long = "linemode-retain-newlines",
        help = "In --line mode, don't chop off trailing \\n from messages"
    )]
    linemode_retain_newlines: bool,

    #[structopt(
        short = "-l", long = "--line", help = "Make each WebSocket message correspond to one line"
    )]
    linemode: bool,
    
    #[structopt(long="origin",help="Add Origin HTTP header to websocket client request")]
    origin: Option<String>,
    
    #[structopt(
        long="header",
        short="H",
        help="Add custom HTTP header to websocket client request. Separate header name and value with a colon and optionally a single space. Can be used multiple times.",
        parse(try_from_str="interpret_custom_header"),
    )]
    custom_headers: Vec<(String,Vec<u8>)>,
    
    #[structopt(long="websocket-version", help="Override the Sec-WebSocket-Version value")]
    websocket_version: Option<String>,
    
    #[structopt(long="no-close", short="n", help="Don't send Close message to websocket on EOF")]
    websocket_dont_close: bool,
    
    #[structopt(
        short="1",
        long="one-message", 
        help="Send and/or receive only one message. Use with --no-close and/or -u/-U.",
    )]
    one_message : bool,
    
    // TODO: -v --quiet
}

// TODO: make it byte-oriented/OsStr?
fn interpret_custom_header(x:&str) -> Result<(String,Vec<u8>)> {
    let colon = x.find(':');
    let colon = if let Some(colon) = colon { colon } else {
        Err("Argument to --header must contain `:` character")?
    };
    let hn = &x[0..colon];
    let mut hv = &x[colon+1..];
    if hv.chars().next() == Some(' ') {
        hv = &x[colon+2..];
    }
    Ok((hn.to_owned(), hv.as_bytes().to_vec()))
}

fn longhelp() {
    println!(
        r#"(see also the usual --help message)
    
Positional arguments to websocat are generally called specifiers.
Specifiers are ways to obtain a connection from some string representation (i.e. address).

Specifiers may be argumentless (like `mirror:`), can accept an argument (which
may be some path or socket address, like `tcp:`), or can accept a subspecifier
(like `reuse:` or `autoreconnect:`).

Here is the full list of specifier classes in this WebSocat build:

"#
    );

    fn help1(sc: &SpecifierClass) {
        let n = sc.get_name().replace("Class", "");
        let prefixes = sc
            .get_prefixes()
            .iter()
            .map(|x| format!("`{}`", x))
            .collect::<Vec<_>>()
            .join(", ");
        println!("### {}\n\n* {}", n, prefixes);

        let help = 
            sc
            .help()
            //.lines()
            //.map(|x|format!("    {}",x))
            //.collect::<Vec<_>>()
            //.join("\n")
            ;
        println!("{}\n", help);
    }

    macro_rules! my {
        ($x:expr) => {
            help1(&$x);
        };
    }

    list_of_all_specifier_classes!(my);

    println!(
        r#"
  
  
TODO:
  sctp:
  ssl:

Final example just for fun: wacky mode

    websocat ws-c:ws-l:ws-c:- tcp:127.0.0.1:5678
    
Connect to a websocket using stdin/stdout as a transport,
then accept a websocket connection over the previous websocket used as a transport,
then connect to a websocket using previous step as a transport,
then forward resulting connection to the TCP port.

(Excercise to the reader: manage to make it actually connect to 5678).
"#
    );
}

fn run() -> Result<()> {
    if std::env::args().nth(1).unwrap_or_default() == "--long-help" {
        longhelp();
        return Ok(());
    }

    let mut cmd = Opt::from_args();

    if cmd.longhelp {
        longhelp();
        return Ok(());
    }

    if false
    //    || cmd.oneshot
    {
        Err("This mode is not implemented")?
    }
    
    #[cfg(feature="openssl-probe")] {
        openssl_probe::init_ssl_cert_env_vars();
    }

    let mut opts = {
        macro_rules! opts {
            ($($o:ident)*) => {
                Options {
                    $($o : cmd.$o,)*
                }
            };
        }
        opts!(
            websocket_text_mode
            websocket_protocol
            udp_oneshot_mode
            unidirectional
            unidirectional_reverse
            exit_on_eof
            oneshot
            unlink_unix_socket
            exec_args
            ws_c_uri
            linemode_retain_newlines
            origin
            custom_headers
            websocket_version
            websocket_dont_close
            one_message
        )
    };

    let (s1, s2) = if let Some(ref cmds2) = cmd.s2 {
        (spec(&cmd.s1)?, spec(cmds2)?)
    } else {
        if ! (cmd.s1.starts_with("ws://") || cmd.s1.starts_with("wss://")) {
            Err("Simple one-argument mode is only for connecting to websockets. Use explicit two-argument websocat invocation for advanced features.")?;
        }
        // Easy mode
        cmd.linemode = true;
        opts.websocket_text_mode = true;
        if opts.websocket_protocol == None {
            opts.websocket_protocol = Some("tcp".to_owned());
        }
        (spec("-")?, spec(&cmd.s1)?)
    };

    let mut websocat = WebsocatConfiguration { opts, s1, s2 };

    if cmd.linemode {
        use websocat::lints::AutoInstallLinemodeConcern::*;
        websocat = match websocat.auto_install_linemode() {
            Ok(x) => x,
            Err((NoWebsocket,_)) => Err("No websocket usage is specified. Use line2msg: and msg2line: specifiers manually if needed.")?,
            Err((MultipleWebsocket,_)) => Err("Multiple websocket usages are specified. Use line2msg: and msg2line: specifiers manually if needed.")?,
            Err((AlreadyLine,_)) => Err("Can't auto-insert msg2line:/line2msg: if you have already manually specified some of them")?,
        }
    }

    while let Some(concern) = websocat.get_concern() {
        use websocat::lints::ConfigurationConcern::*;
        if concern == StdinToStdout {
            if cmd.dumpspec {
                println!("cat mode");
                return Ok(());
            }

            // Degenerate mode: just copy stdin to stdout and call it a day
            ::std::io::copy(&mut ::std::io::stdin(), &mut ::std::io::stdout())?;
            return Ok(());
        }

        if concern == DegenerateMode {
            if cmd.dumpspec {
                println!("noop");
            }
            return Ok(());
        }

        if concern == StdioConflict {
            Err("Too many usages of stdin/stdout")?;
        }

        if concern == NeedsStdioReuser {
            eprintln!("Warning: replies on stdio get directed at random connected client");
            websocat = websocat.auto_install_reuser();
            continue;
        }

        if concern == NeedsStdioReuser2 {
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
        return Ok(());
    }

    let mut core = Core::new()?;

    let prog = websocat.serve(
        core.handle(),
        std::rc::Rc::new(|e| {
            eprintln!("websocat: {}", e);
        }),
    );
    core.run(prog).map_err(|()| "error running".to_string())?;
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
