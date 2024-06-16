use argh::FromArgs;

#[derive(FromArgs)]
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
    pub dump_spec_early: bool,

    /// execute specified file as Rhai script (e.g. resutling from --dump-spec option output)
    #[argh(switch, short='x')]
    pub scenario: bool,

    /// use text mode (one line = one WebSocket text message)
    #[argh(switch, short='t')]
    pub text: bool,

    /// use binary mode (arbitrary byte chunk = one WebSocket binary message)
    #[argh(switch, short='b')]
    pub binary: bool,

/*
    /// whether or not to jump
    #[argh(switch, short = 'j')]
    jump: bool,

    /// how high to go
    #[argh(option)]
    height: usize,

    /// an optional nickname for the pilot
    #[argh(option)]
    pilot_nickname: Option<String>,
     */
}

//let up: GoUp = argh::from_env();
