#[macro_use]
extern crate websocat;

extern crate futures;
extern crate tokio_core;
extern crate tokio_stdin_stdout;

extern crate env_logger;

#[cfg(feature = "openssl-probe")]
extern crate openssl_probe;

#[macro_use]
extern crate structopt;

use structopt::StructOpt;

use tokio_core::reactor::Core;

use websocat::{Options, SpecifierClass, WebsocatConfiguration1};

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
",
    usage = "websocat ws://URL | wss://URL               (simple client)\n    websocat -s port                            (simple server)\n    websocat [FLAGS] [OPTIONS] <addr1> <addr2>  (advanced mode)"
)]
struct Opt {
    /// In simple mode, WebSocket URL to connect.
    /// In advanced mode first address (there are many kinds of addresses) to use.
    /// See --long-help for info about address types.
    /// If this is an address for listening, it will try serving multiple connections.
    addr1: String,
    /// In advanced mode, second address to connect.
    /// If this is an address for listening, it will accept only one connection.
    addr2: Option<String>,

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
        short = "t", long = "text", help = "Send message to WebSockets as text messages"
    )]
    websocket_text_mode: bool,
    
    #[structopt(
        short = "b", long = "binary", help = "Send message to WebSockets as binary messages"
    )]
    websocket_binary_mode: bool,

    #[structopt(
        long = "oneshot", help = "Serve only once. Not to be confused with -1 (--one-message)"
    )]
    oneshot: bool,

    #[structopt(
        long = "long-help",
        help = "Show the full help message, including list of all address types and advanced flags and options which are normally hidden from help (they have `[A]` marker in their help messages)."
    )]
    longhelp: bool,

    #[structopt(short = "h", long = "help", help = "Short short help message")]
    shorthelp: bool,

    #[structopt(
        long = "dump-spec",
        help = "[A] Instead of running, dump the specifiers representation to stdout"
    )]
    dumpspec: bool,

    #[structopt(long = "protocol", help = "Specify Sec-WebSocket-Protocol: header")]
    websocket_protocol: Option<String>,

    #[structopt(long = "udp-oneshot", help = "[A] udp-listen: replies only one packet per client")]
    udp_oneshot_mode: bool,

    #[structopt(long = "unlink", help = "[A] Unlink listening UNIX socket before binding to it")]
    unlink_unix_socket: bool,

    #[structopt(
        long = "exec-args",
        raw(allow_hyphen_values = r#"true"#),
        help = "[A] Arguments for the `exec:` specifier. Must be the last option, everything after it gets into the exec args list."
    )]
    exec_args: Vec<String>,

    #[structopt(
        long = "ws-c-uri",
        help = "[A] URI to use for ws-c: specifier",
        default_value = "ws://0.0.0.0/"
    )]
    ws_c_uri: String,

    #[structopt(
        long = "linemode-strip-newlines",
        help = "[A] Don't include trailing \\n or \\r\\n coming from streams in WebSocket messages"
    )]
    linemode_strip_newlines: bool,

    #[structopt(
        long = "--no-line", help = "[A] Don't automatically insert line-to-message transformation"
    )]
    no_auto_linemode: bool,

    #[structopt(long = "origin", help = "Add Origin HTTP header to websocket client request")]
    origin: Option<String>,

    #[structopt(
        long = "header",
        short = "H",
        help = "Add custom HTTP header to websocket client request. Separate header name and value with a colon and optionally a single space. Can be used multiple times.",
        parse(try_from_str = "interpret_custom_header")
    )]
    custom_headers: Vec<(String, Vec<u8>)>,

    #[structopt(long = "websocket-version", help = "Override the Sec-WebSocket-Version value")]
    websocket_version: Option<String>,

    #[structopt(
        long = "no-close", short = "n", help = "Don't send Close message to websocket on EOF"
    )]
    websocket_dont_close: bool,

    #[structopt(
        short = "1",
        long = "one-message",
        help = "Send and/or receive only one message. Use with --no-close and/or -u/-U."
    )]
    one_message: bool,
    
    #[structopt(
        short = "s",
        long = "server-mode",
        help = "Simple server mode: specify TCP port or addr:port as single argument"
    )]
    server_mode: bool,
    
    
    #[structopt(
        long = "no-fixups",
        help = "[A] Don't perform automatic command-line fixups. May destabilize websocat operation. Use --dump-spec without --no-fixups to discover what is being inserted automatically and read the full manual about Websocat internal workings.",
    )]
    no_lints: bool,
    
    #[structopt(
        short = "B",
        long = "buffer-size",
        help = "Maximum message size, in bytes",
        default_value = "65536",
    )]
    buffer_size: usize,
    
    #[structopt(
        short = "v",
        parse(from_occurrences),
        help = "Increase verbosity level to info or further"
    )]
    verbosity: u8,
    #[structopt(
        short = "q", 
        help="Suppress all diagnostic messages, except of startup errors",
    )]
    quiet: bool,
}

// TODO: make it byte-oriented/OsStr?
fn interpret_custom_header(x: &str) -> Result<(String, Vec<u8>)> {
    let colon = x.find(':');
    let colon = if let Some(colon) = colon {
        colon
    } else {
        Err("Argument to --header must contain `:` character")?
    };
    let hn = &x[0..colon];
    let mut hv = &x[colon + 1..];
    if hv.starts_with(' ') {
        hv = &x[colon + 2..];
    }
    Ok((hn.to_owned(), hv.as_bytes().to_vec()))
}

// https://github.com/rust-lang/rust/issues/51942
#[cfg_attr(feature = "cargo-clippy", allow(nonminimal_bool))]
fn shorthelp() {
    //use std::io::Write;
    use std::io::{BufRead, BufReader};
    let mut b = vec![];
    if Opt::clap().write_help(&mut b).is_err() {
        eprintln!("Error displaying the help message");
    }
    let mut lines_to_display = vec![];
    let mut do_display = true;
    #[allow(non_snake_case)]
    let mut special_A_permit = false;
    for l in BufReader::new(&b[..]).lines() {
        if let Ok(l) = l {
            {
                let lt = l.trim();
                let new_paragraph_start = false || lt.starts_with('-') || l.is_empty();
                if lt.starts_with("--long-help") {
                    special_A_permit = true;
                }
                if l.contains("[A]") {
                    if special_A_permit {
                        special_A_permit = false;
                    } else {
                        do_display = false;
                        if l.trim().starts_with("[A]") {
                            // Also retroactively retract the previous line
                            let nl = lines_to_display.len() - 1;
                            lines_to_display.truncate(nl);
                        }
                    }
                } else if new_paragraph_start {
                    do_display = true;
                };
            }
            let mut additional_line = None;

            if l == "FLAGS:" {
                additional_line = Some("    (some flags are hidden, see --long-help)".to_string());
            };
            if l == "OPTIONS:" {
                additional_line =
                    Some("    (some options are hidden, see --long-help)".to_string());
            };

            if do_display {
                lines_to_display.push(l);
                if let Some(x) = additional_line {
                    lines_to_display.push(x);
                }
            };
        }
    }
    for l in lines_to_display {
        println!("{}", l);
    }
    //let _ = std::io::stdout().write_all(&b);
}

fn longhelp() {
    //let q = Opt::from_iter(vec!["-"]);
    let mut a = Opt::clap();

    let _ = a.print_help();

    // TODO: promote first alias to title
    println!(
        r#"
    
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

// Based on https://github.com/rust-clique/clap-verbosity-flag/blob/master/src/lib.rs
mod logging {
    
    extern crate log;
    extern crate env_logger;
    
    use self::log::{Level};
    use self::env_logger::Builder as LoggerBuilder;

/// Easily add a `--ver

    pub fn setup_env_logger(ll : u8) -> Result<(), Box<::std::error::Error>> {
        if ::std::env::var("RUST_LOG").is_ok() {
            if ll > 0 {
                eprintln!("websocat: RUST_LOG environment variable overrides any -v");
            }
            env_logger::init();
            return Ok(());
        }
    
        let lf =  match ll {
            //0 => Level::Error,
            0 => Level::Warn,
            1 => Level::Info,
            2 => Level::Debug,
            _ => Level::Trace,
        }.to_level_filter();
        
        LoggerBuilder::new()
            .filter(Some("websocat"), lf)
            .filter(None, Level::Warn.to_level_filter())
            .try_init()?;
        Ok(())
    }

}

fn run() -> Result<()> {
    if std::env::args().nth(1).unwrap_or_default() == "--long-help" {
        longhelp();
        return Ok(());
    }
    if vec!["-?", "-h", "--help"].contains(&std::env::args().nth(1).unwrap_or_default().as_str()) {
        shorthelp();
        return Ok(());
    }

    let mut cmd = Opt::from_args();
    
    let mut quiet = cmd.quiet;

    if cmd.longhelp {
        longhelp();
        return Ok(());
    }

    if cmd.shorthelp {
        shorthelp();
        return Ok(());
    }

    let mut recommend_explicit_text_or_bin = false;

    if cmd.websocket_binary_mode && cmd.websocket_text_mode {
        Err("--binary and --text are mutually exclusive")?;
    }
    if !cmd.websocket_binary_mode && !cmd.websocket_text_mode {
        cmd.websocket_text_mode = true;
        recommend_explicit_text_or_bin = true;
    }

    if false
    //    || cmd.oneshot
    {
        Err("This mode is not implemented")?
    }

    #[cfg(feature = "openssl-probe")]
    {
        openssl_probe::init_ssl_cert_env_vars();
    }

    let mut opts : Options = Default::default();
    {
        macro_rules! opts {
            ($($o:ident)*) => {{
                $(opts.$o = cmd.$o;)*
            }};
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
            linemode_strip_newlines
            origin
            custom_headers
            websocket_version
            websocket_dont_close
            one_message
            no_auto_linemode
            buffer_size
        )
    };

    let (s1, s2) : (String,String) = if let Some(cmds2) = cmd.addr2 {
        // Advanced mode
        if cmd.server_mode {
            Err("--server and two positional arguments are incompatible.\nBuild server command line without -s option, but with `listen` address types")?
        }
        (cmd.addr1, cmds2)
    } else {
        // Easy mode
        recommend_explicit_text_or_bin = false;
        if cmd.server_mode {
            if cmd.addr1.contains(':') {
                if !quiet {
                    eprintln!("Listening on ws://{}/", cmd.addr1);
                }
                (format!("ws-l:{}", cmd.addr1), "-".to_string())
            } else {
                if !quiet {
                    eprintln!("Listening on ws://127.0.0.1:{}/", cmd.addr1);
                }
                (format!("ws-l:127.0.0.1:{}", cmd.addr1), "-".to_string())
            }
        } else {
            if !(cmd.addr1.starts_with("ws://") || cmd.addr1.starts_with("wss://")) {
                if !quiet {
                    eprintln!("Specify ws:// or wss:// URI to connect to a websocket");
                }
                Err("Invalid command-line parameters")?;
            }
            ("-".to_string(), cmd.addr1)
        }
    };
    

    let websocat1 = WebsocatConfiguration1 { opts, addr1:s1, addr2:s2 };
    let mut websocat2 = websocat1.parse1()?;
    
    if websocat2.inetd_mode() {
        quiet = true;
    }
    
    if !quiet && recommend_explicit_text_or_bin {
        eprintln!("It is recommended to either set --binary or --text explicitly");
    }
    if !quiet {
        logging::setup_env_logger(cmd.verbosity)?;
    }
    
    if ! cmd.no_lints {
        websocat2.lint_and_fixup(std::rc::Rc::new(move |e:&str| {
            if !quiet {
                eprintln!("{}", e);
            }
        }))?;
    }
    let websocat = websocat2.parse2()?;

    if cmd.dumpspec {
        println!("{:?}", websocat.s1);
        println!("{:?}", websocat.s2);
        println!("{:?}", websocat.opts);
        return Ok(());
    }

    let mut core = Core::new()?;

    let prog = websocat.serve(
        core.handle(),
        std::rc::Rc::new(move |e| {
            if !quiet {
                eprintln!("websocat: {}", e);
            }
        }),
    );
    core.run(prog).map_err(|()| "error running".to_string())?;
    Ok(())
}

fn main() {
    let r = run();

    if let Err(e) = r {
        eprintln!("websocat: {}", e);
        ::std::process::exit(1);
    }
}
