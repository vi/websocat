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
use websocat::options::StaticFile;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

#[derive(StructOpt, Debug)]
#[structopt(
    after_help = "
Basic examples:
  Command-line websocket client:
    websocat ws://echo.websocket.org/
    
  WebSocket server
    websocat -s 8080
    
  WebSocket-to-TCP proxy:
    websocat --binary ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    
",
    usage = "websocat ws://URL | wss://URL               (simple client)\n    websocat -s port                            (simple server)\n    websocat [FLAGS] [OPTIONS] <addr1> <addr2>  (advanced mode)"
)]
struct Opt {
    /// In simple mode, WebSocket URL to connect.
    /// In advanced mode first address (there are many kinds of addresses) to use.
    /// See --help=types for info about address types.
    /// If this is an address for listening, it will try serving multiple connections.
    addr1: Option<String>,
    /// In advanced mode, second address to connect.
    /// If this is an address for listening, it will accept only one connection.
    addr2: Option<String>,

    #[structopt(
        short = "u", long = "unidirectional", help = "Inhibit copying data in one direction"
    )]
    unidirectional: bool,
    #[structopt(
        short = "U",
        long = "unidirectional-reverse",
        help = "Inhibit copying data in the other direction (or maybe in both directions if combined with -u)"
    )]
    unidirectional_reverse: bool,

    #[structopt(
        long = "exit-on-eof",
        short = "E",
        help = "Close a data transfer direction if the other one reached EOF"
    )]
    exit_on_eof: bool,

    #[structopt(short = "t", long = "text", help = "Send message to WebSockets as text messages")]
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
        short = "h",
        long = "help",
        help = "See the help.\n--help=short is the list of easy options and address types\n--help=long lists all options and types (see [A] markers)\n--help=doc also shows longer description and examples."
    )]
    help: Option<String>,

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
        help = "[A] URI to use for ws-c: overlay",
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
        help = "[A] Don't perform automatic command-line fixups. May destabilize websocat operation. Use --dump-spec without --no-fixups to discover what is being inserted automatically and read the full manual about Websocat internal workings."
    )]
    no_lints: bool,

    #[structopt(
        short = "B",
        long = "buffer-size",
        help = "Maximum message size, in bytes",
        default_value = "65536"
    )]
    buffer_size: usize,

    #[structopt(
        short = "v", parse(from_occurrences), help = "Increase verbosity level to info or further"
    )]
    verbosity: u8,

    #[structopt(short = "q", help = "Suppress all diagnostic messages, except of startup errors")]
    quiet: bool,

    #[structopt(
        long = "queue-len",
        help = "[A] Number of pending queued messages for broadcast reuser",
        default_value = "16"
    )]
    broadcast_queue_len: usize,

    #[structopt(
        short = "S",
        long = "strict",
        help = "strict line/message mode: drop too long messages instead of splitting them, drop incomplete lines."
    )]
    strict_mode: bool,

    #[structopt(
        short = "0", long = "null-terminated", help = "Use \\0 instead of \\n for linemode"
    )]
    linemode_zero_terminated: bool,

    #[structopt(
        long = "restrict-uri",
        help = "When serving a websocket, only accept the given URI, like `/ws`\nThis liberates other URIs for things like serving static files or proxying."
    )]
    restrict_uri: Option<String>,
    
    #[structopt(
        short = "F",
        long = "static-file",
        help = "Serve a named static file for non-websocket connections.\nArgument syntax: <URI>:<Content-Type>:<file-path>\nArgument example: /index.html:text/html:index.html\nDirectories are not and will not be supported for security reasons.\nCan be specified multiple times.",
        parse(try_from_str = "interpret_static_file")
    )]
    serve_static_files: Vec<StaticFile>,
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

fn interpret_static_file(x: &str) -> Result<StaticFile> {
    let colon1 = match x.find(':') {
        Some(x) => x,
        None => Err("Argument to --static-file must contain two colons (`:`)")?
    };
    let uri = &x[0..colon1];
    let rest = &x[colon1+1..];
    let colon2 = match rest.find(':') {
        Some(x) => x,
        None => Err("Argument to --static-file must contain two colons (`:`)")?
    };
    let ct = &rest[0..colon2];
    let fp = &rest[colon2+1..];
    if uri.is_empty() || ct.is_empty() || fp.is_empty() {
        Err("Empty URI, content-type or path in --static-file parameter")?
    }
    Ok(StaticFile{
        uri: uri.to_string(), 
        content_type: ct.to_string(),
        file: fp.into(),
    })
}

pub mod help;

// Based on https://github.com/rust-clique/clap-verbosity-flag/blob/master/src/lib.rs
mod logging {

    extern crate env_logger;
    extern crate log;

    use self::env_logger::Builder as LoggerBuilder;
    use self::log::Level;

    pub fn setup_env_logger(ll: u8) -> Result<(), Box<::std::error::Error>> {
        if ::std::env::var("RUST_LOG").is_ok() {
            if ll > 0 {
                eprintln!("websocat: RUST_LOG environment variable overrides any -v");
            }
            env_logger::init();
            return Ok(());
        }

        let lf = match ll {
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
        help::longhelp();
        return Ok(());
    }
    if vec!["-?", "-h", "--help"].contains(&std::env::args().nth(1).unwrap_or_default().as_str()) {
        help::shorthelp();
        return Ok(());
    }

    let mut cmd = Opt::from_args();

    let mut quiet = cmd.quiet;

    if let Some(h) = cmd.help {
        if &h == "long" || &h == "full" {
            help::longhelp();
            return Ok(());
        } else if &h == "doc" {
            help::dochelp();
            return Ok(());
        }

        help::shorthelp();
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
    
    //if !cmd.serve_static_files.is_empty() && cmd.restrict_uri.is_none() {
    //    Err("Specify --static-file is not supported without --restrict-uri")?
    //}

    if false
    //    || cmd.oneshot
    {
        Err("This mode is not implemented")?
    }

    #[cfg(feature = "openssl-probe")]
    {
        openssl_probe::init_ssl_cert_env_vars();
    }

    let mut opts: Options = Default::default();
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
            linemode_zero_terminated
            restrict_uri
            serve_static_files
        )
    };

    let (s1, s2): (String, String) = match (cmd.addr1, cmd.addr2) {
        (None, None) => {
            help::shorthelp();
            return Err("No URL specified")?;
        }
        (Some(cmds1), Some(cmds2)) => {
            // Advanced mode
            if cmd.server_mode {
                Err("--server and two positional arguments are incompatible.\nBuild server command line without -s option, but with `listen` address types")?
            }
            (cmds1, cmds2)
        }
        (Some(cmds1), None) => {
            // Easy mode
            recommend_explicit_text_or_bin = false;
            if cmd.server_mode {
                if cmds1.contains(':') {
                    if !quiet {
                        eprintln!("Listening on ws://{}/", cmds1);
                    }
                    (format!("ws-l:{}", cmds1), "-".to_string())
                } else {
                    if !quiet {
                        eprintln!("Listening on ws://127.0.0.1:{}/", cmds1);
                    }
                    (format!("ws-l:127.0.0.1:{}", cmds1), "-".to_string())
                }
            } else {
                if !(cmds1.starts_with("ws://") || cmds1.starts_with("wss://")) {
                    if !quiet {
                        eprintln!("Specify ws:// or wss:// URI to connect to a websocket");
                    }
                    Err("Invalid command-line parameters")?;
                }
                ("-".to_string(), cmds1)
            }
        }
        (None, Some(_)) => unreachable!(),
    };

    if opts.websocket_text_mode {
        opts.read_debt_handling = websocat::readdebt::DebtHandling::Warn;
    }
    if cmd.strict_mode {
        opts.read_debt_handling = websocat::readdebt::DebtHandling::DropMessage;
        opts.linemode_strict = true;
    }

    let websocat1 = WebsocatConfiguration1 {
        opts,
        addr1: s1,
        addr2: s2,
    };
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

    if !cmd.no_lints {
        websocat2.lint_and_fixup(&move |e: &str| {
            if !quiet {
                eprintln!("{}", e);
            }
        })?;
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
