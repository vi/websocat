use websocat_api::anyhow;

fn version() {
    println!("websocat {}", env!("CARGO_PKG_VERSION"));
}

mod help;
use help::{help, HelpMode};

static SHORT_OPTS : [(char, &str); 4] = [
    ('u', "unidirectional"),
    ('U', "unidirectional-reverse"),
    ('V', "version"),
    ('s', "server-mode"),
];

/// Options that do not come from a Websocat classes
static CORE_OPTS : [(&str, &str, &str); 5] = [
    ("version", "", "Show Websocat version"),
    ("help", "[mode]",  "Show Websocat help message. There are four help modes, use --help=help for list them."),
    ("dry-run", "", "Skip actual execution of the constructed node"),
    ("dump-spec", "", "Instead of executing the session, describe its tree to stdout"),
    ("server-mode", "", "Run simple WebSocket server for development on a specified port"),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut treestrings = vec![];
    let mut dryrun = false;
    let mut dumpspec = false;
    let mut servermode = false;

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    let reg = websocat_allnodes::all_node_classes();

    let allopts = reg.get_all_cli_options()?;

    let mut class_induced_cli_opts: std::collections::HashMap<String, _> =
        std::collections::HashMap::new();

    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        let optname = match arg {
            lexopt::Arg::Short(x) => match x {
                '?' => return Ok(help(HelpMode::Short, &reg, &allopts)),
                x => {
                    let mut lo = None;
                    for (short, long) in SHORT_OPTS {
                        if x == short {
                            lo = Some(long);
                            break;
                        }
                    }
                    if lo.is_none() { anyhow::bail!("Unknown short option `{}`", x); }
                    lo.unwrap()
                }
            },
            lexopt::Arg::Long(x) => x,
            lexopt::Arg::Value(x) => {
                treestrings.push(os_str_bytes::OsStrBytes::to_raw_bytes(&*x).into_owned());
                continue
            }
        };
        match optname {
            "dryrun" => dryrun = true,
            "dump-spec" => {dryrun = true; dumpspec = true; }
            "version" => return Ok(version()),
            "help" => {
                let mode = parser.value();
                let hm = if let Ok(m) = mode {
                    match m.to_string_lossy().as_ref() {
                        "short" => HelpMode::Short,
                        "long" => HelpMode::Full,
                        "full" => HelpMode::Full,
                        "manpage" => HelpMode::Man,
                        "markdown" => HelpMode::Markdown,
                        "list" => HelpMode::JustListThings,
                        "help" => return Ok(println!("--help modes: short, full, manpage, markdown, list or some node name")),
                         x => HelpMode::SpecificThing(x.to_owned()),
                    }
                } else {
                    HelpMode::Full
                };
                return Ok(help(hm, &reg, &allopts));
            }
            "server-mode" => servermode = true,
            x => {
                //let b = parser.value()?;
                if let Some(t) = allopts.get(x) {
                    match (&t.typ, t.for_array) {
                        (websocat_api::PropertyValueType::Booly, false) => {
                            class_induced_cli_opts
                                .insert(x.to_owned(), websocat_api::smallvec::smallvec![websocat_api::PropertyValue::Booly(true)]);
                        }
                        _ => todo!(),
                    }
                } else {
                    anyhow::bail!("Unknown long option `{}`", x);
                }
            }
        }
    }

    if treestrings.len() == 2 {
        anyhow::bail!("Two-argument Websocat1-style mode is not yet supported. 
            Use single argument with a special value like `[session left=... right=...]` instead ");
    }
    if treestrings.len() != 1 {
        anyhow::bail!("Exactly one positional argument required");
    }

    let mut main_arg = &treestrings[0];
    let mut complex_arg;

    if servermode {
        complex_arg = Vec::with_capacity(32);
        complex_arg.extend_from_slice(b"[server ");
        complex_arg.extend(main_arg);
        complex_arg.extend_from_slice(b"]");
        main_arg = &complex_arg;
    } else if main_arg.starts_with(b"[") {
        // leave `main_arg` as is
    } else if main_arg.starts_with(b"ws://") || main_arg.starts_with(b"wss://") {
        complex_arg = Vec::with_capacity(32);
        complex_arg.extend_from_slice(b"[client ");
        complex_arg.extend(main_arg);
        complex_arg.extend_from_slice(b"]");
        main_arg = &complex_arg;
    };

    let c = websocat_api::Circuit::build_from_tree_bytes(
        &reg,
        &class_induced_cli_opts,
        main_arg,
    )?;

    if dumpspec {
        println!("{}", websocat_api::StrNode::reverse(c.root, &c.nodes)?);
    }

    if dryrun {
        return Ok(());
    }

    if let Err(e) = c.run_root_node().await {
        eprintln!("Error: {:#}", e);
    }
    Ok(())
}
