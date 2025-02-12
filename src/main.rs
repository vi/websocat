use scenario_executor::{
    scenario::load_scenario,
    types::{Handle, Task},
    utils1::run_task,
};
use scenario_planner::{types::WebsocatInvocation, utils::IdentifierGenerator};

use crate::scenario_planner::types::SpecifierStack;

use clap::Parser;

pub mod scenario_executor {
    pub mod copydata;

    pub mod debugfluff;
    pub mod fluff;
    pub mod http1;
    pub mod linemode;
    pub mod logoverlay;
    pub mod misc;
    #[cfg(feature="ssl")]
    pub mod nativetls;
    pub mod osstr;
    pub mod scenario;
    pub mod subprocess;
    pub mod tcp;
    pub mod trivials1;
    pub mod trivials2;
    pub mod trivials3;
    pub mod types;
    pub mod udp;
    pub mod udpserver;
    #[cfg(unix)]
    pub mod unix;
    pub mod utils1;
    pub mod utils2;
    pub mod wsframer;
    pub mod wswithpings;

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
    pub mod linter;
    pub mod scenarioprinter;
    pub mod types;
    pub mod utils;
}

pub mod cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    //tracing_subscriber::fmt().json().with_max_level(tracing::Level::DEBUG).init();
    tracing_subscriber::fmt::init();

    let mut args = cli::WebsocatArgs::parse();
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
            args.spec2 = Some("stdio:".to_owned().into());
            if !args.binary && !args.text {
                args.text = true;
            }
        }

        if !args.binary && !args.text {
            eprintln!("Using --binary mode by default");
            args.binary = true;
        }
        if args.server {
            let s: &str = args.spec1.as_os_str().try_into()?;
            let mut s = s.to_owned();
            if !s.contains(':') {
                s = format!("127.0.0.1:{}", s);
            }
            s = format!("ws-l:{}", s);
            args.spec1 = s.into();
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

        if !invocation.opts.no_lints {
            for lint in invocation.lints() {
                eprintln!("warning: {lint}");
            }
        }

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
