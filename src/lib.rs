use rand::SeedableRng;
use scenario_executor::{
    scenario::load_scenario,
    types::{Handle, Registry, Task},
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
    pub mod lengthprefixed;
    pub mod linemode;
    pub mod logoverlay;
    pub mod misc;
    pub mod mockbytestream;
    #[cfg(feature = "ssl")]
    pub mod nativetls;
    pub mod osstr;
    pub mod registryconnectors;
    pub mod scenario;
    pub mod subprocess;
    pub mod tcp;
    pub mod trivials1;
    pub mod trivials2;
    pub mod trivials3;
    pub mod dgtools1;
    pub mod types;
    pub mod udp;
    pub mod udpserver;
    #[cfg(unix)]
    pub mod unix1;
    #[cfg(unix)]
    pub mod unix2;
    pub mod utils1;
    pub mod utils2;
    pub mod wsframer;
    pub mod wswithpings;

    pub mod all_functions;

    pub const MAX_CONTROL_MESSAGE_LEN: usize = 65536;
}

pub mod scenario_planner {
    pub mod buildscenario;
    pub mod buildscenario_endpoints;
    pub mod buildscenario_exec;
    pub mod buildscenario_misc;
    pub mod buildscenario_overlays;
    pub mod buildscenario_tcp;
    pub mod buildscenario_udp;
    pub mod buildscenario_unix;
    pub mod buildscenario_ws;
    pub mod fromstr;
    pub mod linter;
    pub mod patcher;
    pub mod scenarioprinter;
    pub mod types;
    pub mod utils;
}

pub mod cli;
pub mod test_utils;

pub async fn websocat_main<I, T, D>(
    argv: I,
    mut diagnostic_output: D,
    time_base: tokio::time::Instant,
    allow_stdout: bool,
    registry: Registry,
) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
    D: std::io::Write + Send + 'static,
{
    let mut args = cli::WebsocatArgs::parse_from(argv);
    let dump_spec = args.dump_spec;

    let prng = if let Some(seed) = args.random_seed {
        rand_chacha::ChaCha12Rng::seed_from_u64(seed)
    } else {
        rand_chacha::ChaCha12Rng::from_os_rng()
    };

    if args.accept_from_fd {
        writeln!(
            diagnostic_output,
            "--accept-from-fd is an obsolete Websocat1 option. Use e.g. `ws-u:unix-l-fd:3` instead."
        )?;
        anyhow::bail!("Invalid option");
    }

    let global_scenario: &str;
    let scenario_file;
    let scenario_built_text;
    if args.scenario {
        if args.spec2.is_some() {
            writeln!(
                diagnostic_output,
                "In --scenario mode only one argument is expected"
            )?;
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
            writeln!(diagnostic_output, "Using --binary mode by default")?;
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

        let left_stack = SpecifierStack::my_from_str(&args.spec1)?;
        let right_stack = SpecifierStack::my_from_str(&args.spec2.take().unwrap())?;

        let mut invocation = WebsocatInvocation {
            left: left_stack,
            right: right_stack,
            opts: args,
            beginning: vec![],
        };

        let mut idgen = IdentifierGenerator::new();

        if !invocation.opts.no_lints {
            for lint in invocation.lints() {
                writeln!(diagnostic_output, "warning: {lint}")?;
            }
        }

        if !invocation.opts.dump_spec_phase1 {
            invocation.patches(&mut idgen)?;
        }

        if invocation.opts.dump_spec_phase1 || invocation.opts.dump_spec_phase2 {
            writeln!(diagnostic_output, "{:#?}", invocation.left)?;
            writeln!(diagnostic_output, "{:#?}", invocation.right)?;
            writeln!(diagnostic_output, "{:#?}", invocation.opts)?;
            writeln!(diagnostic_output, "{:#?}", invocation.beginning)?;
            return Ok(());
        }

        scenario_built_text = invocation.build_scenario(&mut idgen)?;
        global_scenario = &scenario_built_text;

        if dump_spec {
            if allow_stdout {
                println!("{}", global_scenario);
            } else {
                writeln!(diagnostic_output, "{}", global_scenario)?;
            }
            return Ok(());
        }
    }

    let ctx = load_scenario(
        global_scenario,
        Box::new(diagnostic_output),
        time_base,
        Box::new(prng),
        registry,
    )?;
    let task: Handle<Task> = ctx.execute()?;
    run_task(task).await;

    Ok(())
}
