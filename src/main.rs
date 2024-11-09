use scenario_executor::{
    scenario::load_scenario,
    types::{Handle, Task},
    utils::run_task,
};
use scenario_planner::{types::WebsocatInvocation, utils::IdentifierGenerator};

use crate::scenario_planner::types::SpecifierStack;

use clap::Parser;

pub mod scenario_executor {
    pub mod copydata;

    pub mod debugfluff;
    pub mod fluff;
    pub mod http1;
    pub mod misc;
    pub mod nativetls;
    pub mod scenario;
    pub mod tcp;
    pub mod udp;
    pub mod udpserver;
    pub mod trivials1;
    pub mod trivials2;
    pub mod types;
    pub mod utils;
    pub mod wsframer;
    pub mod wswithpings;
    pub mod linemode;
    pub mod logoverlay;
    pub mod subprocess;
    pub mod osstr;
    pub mod unix;

    pub mod all_functions;
}

pub mod scenario_planner {
    pub mod buildscenario;
    pub mod buildscenario_exec;
    pub mod buildscenario_tcp;
    pub mod buildscenario_udp;
    pub mod buildscenario_unix;
    pub mod buildscenario_ws;
    pub mod fromstr;
    pub mod patcher;
    pub mod scenarioprinter;
    pub mod types;
    pub mod utils;
}

pub mod cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    //tracing_subscriber::fmt().json().with_max_level(tracing::Level::DEBUG).init();
    tracing_subscriber::fmt::init();

    let mut args =  cli::WebsocatArgs::parse();
    let dump_spec = args.dump_spec;

    let global_scenario: &str;
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
            args.spec2 = Some("stdio:".to_owned());
            if !args.binary && !args.text {
                args.text = true;
            }
        }

        if !args.binary && !args.text {
            eprintln!("Using --binary mode by default");
            args.binary = true;
        }
        if args.server {
            if !args.spec1.contains(':') {
                args.spec1 = format!("127.0.0.1:{}", args.spec1);
            }
            args.spec1 = format!("ws-l:{}", args.spec1);
        }

        let left_stack = SpecifierStack::from_str(&args.spec1)?;
        let right_stack = SpecifierStack::from_str(&args.spec2.take().unwrap())?;

        let mut invocation = WebsocatInvocation {
            left: left_stack,
            right: right_stack,
            opts: args,
            beginning: vec![],
        };

        let mut idgen = IdentifierGenerator::new();

        if !invocation.opts.dump_spec_phase1 {
            invocation.patches(&mut idgen)?;
        }

        if invocation.opts.dump_spec_phase1 || invocation.opts.dump_spec_phase2 {
            println!("{:#?}", invocation.left);
            println!("{:#?}", invocation.right);
            println!("{:#?}", invocation.opts);
            println!("{:#?}", invocation.beginning);
            return Ok(());
        }

        scenario_built_text = invocation.build_scenario(&mut idgen)?;
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
