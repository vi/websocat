use scenario_executor::{scenario::load_scenario, types::{Handle, Task}, utils::run_task};
use scenario_planner::types::WebsocatInvocation;

use crate::scenario_planner::types::SpecifierStack;



pub mod scenario_executor {
    pub mod copydata;

    pub mod misc;
    pub mod trivials1;
    pub mod trivials2;
    pub mod types;
    pub mod fluff;
    pub mod tcp;
    pub mod scenario;
    pub mod debugfluff;
    pub mod utils;
    pub mod wsupgrade;
    pub mod wsframer;
    pub mod wswithpings;

    pub mod all_functions;
}

pub mod scenario_planner {
    pub mod types;
    pub mod fromstr;
    pub mod buildscenario;
    pub mod scenarioprinter;
    pub mod patcher;
}

pub mod cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    //tracing_subscriber::fmt().json().with_max_level(tracing::Level::DEBUG).init();
    tracing_subscriber::fmt::init();

    let mut args : cli::WebsocatArgs = argh::from_env();
    let dump_spec = args.dump_spec;

    let global_scenario : &str;
    let scenario_file;
    let scenario_built_text;
    if args.scenario {
        if args.spec2.is_some() {
            eprintln!("In --scenario mode only one argument is expected");
        }

        scenario_file = std::fs::read(args.spec1)?;
        global_scenario = std::str::from_utf8(&scenario_file[..])?;
    } else {
        if args.spec2.is_none() {
            anyhow::bail!("Unimplemented");
        }

        if !args.binary || !args.text {
            eprintln!("Using --binary mode by default");
            args.binary = true;
        }

        let left_stack = SpecifierStack::from_str(&args.spec1)?;
        let right_stack = SpecifierStack::from_str(&args.spec2.take().unwrap())?;

        let mut invocation = WebsocatInvocation {
            left: left_stack,
            right: right_stack,
            opts: args,
        };

        invocation.patches()?;

        if invocation.opts.dump_spec_early {
            println!("{:#?}", invocation.left);
            println!("{:#?}", invocation.right);
            println!("{:#?}", invocation.opts);
            return Ok(());
        }

        scenario_built_text = invocation.build_scenario()?;
        global_scenario = &scenario_built_text;

        if dump_spec {
            println!("{}", global_scenario);
            return Ok(());
        }
    }


    let ctx = load_scenario(global_scenario)?;
    let task: Handle<Task> = ctx.execute()?;
    run_task(task).await;

    Ok(())
}

