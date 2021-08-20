
use websocat_api::anyhow;


#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut from_str_mode = false;

    let mut treestrings = vec![];
    let mut enable_forward = true;
    let mut enable_backward = true;
    let mut dryrun = false;

    tracing_subscriber::fmt::init();
    
    let reg = websocat_allnodes::all_node_classes();

    let allopts = reg.get_all_cli_options()?;

    let mut class_induced_cli_opts : std::collections::HashMap<String, _> = std::collections::HashMap::new();

    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            lexopt::Arg::Short(x) => match x {
                'u' => enable_backward = false,
                'U' => enable_forward = false,
                _ => anyhow::bail!("Unknown short option `{}`", x),
            }
            lexopt::Arg::Long(x) => match x {
                "str" => from_str_mode = true,
                "dryrun" => dryrun = true,
                x => {
                    //let b = parser.value()?;
                    if let Some(t) = allopts.get(x) {
                        match t {
                            websocat_api::PropertyValueType::Booly => {
                                class_induced_cli_opts.insert(x.to_owned(), websocat_api::PropertyValue::Booly(true));
                            }
                            _ => todo!(),
                        }
                    } else {
                        anyhow::bail!("Unknown long option `{}`", x);
                    }
                }
            }
            lexopt::Arg::Value(x) => {
                treestrings.push(os_str_bytes::OsStrBytes::to_raw_bytes(&*x).into_owned())
            }
        }
    }

    if from_str_mode {
        return Ok(());
    }

    if treestrings.len() != 2 {
        anyhow::bail!("Exactly two positional arguments required");
    }

    let c = websocat_api::Session::build_from_two_tree_bytes(
        &reg, 
        &class_induced_cli_opts,
        &treestrings[0],
        &treestrings[1],
    )?;

    println!("{}", websocat_api::StrNode::reverse(c.left, &c.nodes)?);
    println!("{}", websocat_api::StrNode::reverse(c.right, &c.nodes)?);
    

    let opts = websocat_session::Opts {
        enable_forward,
        enable_backward,
    };

    if dryrun {
        return Ok(());
    }

    if let Err(e) = websocat_session::run(opts, c).await {
        eprintln!("Error: {:#}", e);
    }
    Ok(())
}

