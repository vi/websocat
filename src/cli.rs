use argh::FromArgs;

#[derive(FromArgs, Debug)]
/// Tool to connect to WebSocket, listen them and do other network tricks
pub struct WebsocatArgs {
    #[argh(positional)]
    pub spec1: String,

    #[argh(positional)]
    pub spec2: Option<String>,

    /// do not execute this Websocat invocation, print equivalent Rhai script instead.
    #[argh(switch)]
    pub dump_spec: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    #[argh(switch)]
    pub dump_spec_phase1: bool,

    /// do not execute this Websocat invocation, print debug representation of specified arguments.
    #[argh(switch)]
    pub dump_spec_phase2: bool,

    /// execute specified file as Rhai script (e.g. resutling from --dump-spec option output)
    #[argh(switch, short = 'x')]
    pub scenario: bool,

    /// use text mode (one line = one WebSocket text message)
    #[argh(switch, short = 't')]
    pub text: bool,

    /// use binary mode (arbitrary byte chunk = one WebSocket binary message)
    #[argh(switch, short = 'b')]
    pub binary: bool,

    /// resolve hostnames to IP addresses late (every time when forwarding a connection) instead of one time at the beginning
    #[argh(switch)]
    pub late_resolve: bool,

    /// accept invalid domains and root certificates for TLS client connections
    #[argh(switch, short = 'k')]
    pub insecure: bool,

    /// manually specify domain for `tls:` overlay or override domain for `wss://` URLs
    #[argh(option)]
    pub tls_domain: Option<String>,

    /// listen for WebSocket conenctions instead of establishing client WebSocket connection
    #[argh(switch, short = 's')]
    pub server: bool,
    
    /// log more data from `log:` overlay
    #[argh(switch)]
    pub log_verbose: bool,

    /// do not log full content of the data from `log:` overlay, just chunk lengths
    #[argh(switch)]
    pub log_omit_content: bool,

    /// use hex lines instead of escaped characters for `log:`` overlay.
    #[argh(switch)]
    pub log_hex: bool,

    /// automatically insert `log:` overlay in an apprioriate place to debug issues by displaying traffic chunks
    #[argh(switch)]
    pub log_traffic: bool,

    /// URI for `ws-c:` overlay.
    #[argh(option)]
    pub ws_c_uri: Option<String>,
}
