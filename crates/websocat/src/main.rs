use websocat_api::anyhow;

fn version() {
    println!("websocat {}", env!("CARGO_PKG_VERSION"));
}

mod help;
use help::{help, HelpMode};

static SHORT_OPTS : [(char, &str); 3] = [
    ('u', "unidirectional"),
    ('U', "unidirectional-reverse"),
    ('V', "version"),
];

/// Options that do not come from a Websocat classes
static CORE_OPTS : [(&str, &str, &str); 5] = [
    ("version", "", "Show Websocat version"),
    ("help", "[mode]",  "Show Websocat help message. There are four help modes, use --help=help for list them."),
    ("str", "", "???"),
    ("dryrun", "", "???"),
    ("dump-spec", "", "Instead of executing the session, describe its tree to stdout")
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut from_str_mode = false;

    let mut treestrings = vec![];
    let mut dryrun = false;

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
            "str" => from_str_mode = true,
            "dryrun" => dryrun = true,
            "dump-spec" => dryrun = true,
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

    if from_str_mode {
        return Ok(());
    }

    if treestrings.len() != 1 {
        anyhow::bail!("Exactly one positional argument required");
    }

    let c = websocat_api::Circuit::build_from_tree_bytes(
        &reg,
        &class_induced_cli_opts,
        &treestrings[0],
    )?;

    if dryrun {
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
