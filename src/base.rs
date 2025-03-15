use std::ffi::OsString;

use super::{cli::{WebsocatArgs, WebsocatGlobalArgs}, composed_cli::ComposedArgument};
use itertools::Itertools;
use rand::SeedableRng;
use super::scenario_executor::{
    scenario::load_scenario,
    types::{Handle, Registry, Task},
    utils1::run_task,
};
use super::scenario_planner::{
    types::{ScenarioPrinter, WebsocatInvocation},
    utils::IdentifierGenerator,
};

use crate::scenario_planner::types::{SpecifierPosition, SpecifierStack};

use clap::Parser;


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
    let mut argv = argv.into_iter().multipeek();

    let mut global_args = WebsocatGlobalArgs::default();

    let global_scenario: &str;
    let scenario_file;
    let scenario_built_text;

    let _zeroeth_arg = argv.peek();
    let first_arg: OsString = argv.peek().map(|x| x.clone().into()).unwrap_or_default();
    let second_arg: OsString = argv.peek().map(|x| x.clone().into()).unwrap_or_default();
    let compose_mode = {
        if first_arg == "--compose" {
            if second_arg == "--dump-spec-phase0" {
                global_args.dump_spec_phase0 = true;
            }
            true
        } else {
            false
        }
    };

    if !compose_mode {
        let args = WebsocatArgs::parse_from(argv);
        if args.dump_spec_phase0 {
            writeln!(diagnostic_output, "{:?}", args)?;
        }
        global_args.hoover(&args)?;

        if let Some(scenario_filename) = global_args.scenario {
            if compose_mode {
                anyhow::bail!("--scenario and --compose are incompatible");
            }
            scenario_file = std::fs::read(scenario_filename)?;
            global_scenario = std::str::from_utf8(&scenario_file[..])?;
        } else {
            let mut idgen = IdentifierGenerator::new();
            let mut printer = ScenarioPrinter::new();

            let mut invocation = WebsocatInvocation::from_args(args, &mut diagnostic_output)?;
            if invocation.prepare(&mut diagnostic_output, &mut idgen)? {
                return Ok(());
            }

            invocation.print_scenario(&mut idgen, &mut printer)?;
            scenario_built_text = printer.into_result();
            global_scenario = &scenario_built_text;
        }
    } else {
        argv.next();
        argv.next();

        let composed = super::composed_cli::parse(argv)?;

        if global_args.dump_spec_phase0 {
            writeln!(diagnostic_output, "{:?}", composed)?;
            return Ok(());
        }

        let mut idgen = IdentifierGenerator::new();
        let mut printer = ScenarioPrinter::new();

        composed.print(
            &mut printer,
            &mut idgen,
            &mut global_args,
            &mut diagnostic_output,
        )?;

        scenario_built_text = printer.into_result();
        global_scenario = &scenario_built_text;
    }

    if global_args.dump_spec {
        if allow_stdout {
            println!("{}", global_scenario);
        } else {
            writeln!(diagnostic_output, "{}", global_scenario)?;
        }
        return Ok(());
    }

    let prng = if let Some(seed) = global_args.random_seed {
        rand_chacha::ChaCha12Rng::seed_from_u64(seed)
    } else {
        rand_chacha::ChaCha12Rng::from_os_rng()
    };

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

impl WebsocatArgs {
    pub fn cook_args(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

impl WebsocatInvocation {
    pub fn from_args<D>(
        mut args: WebsocatArgs,
        diagnostic_output: &mut D,
    ) -> anyhow::Result<Self>
    where
        D: std::io::Write + Send + 'static,
    {
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

        let left_stack = SpecifierStack::my_from_str(&args.spec1, SpecifierPosition::Left)?;
        let right_stack = SpecifierStack::my_from_str(&args.spec2.take().unwrap(), SpecifierPosition::Right)?;

        Ok(WebsocatInvocation {
            left: left_stack,
            right: right_stack,
            opts: args,
            beginning: vec![],
        })
    }

    pub fn prepare<D>(
        &mut self,
        diagnostic_output: &mut D,
        vars: &mut IdentifierGenerator,
    ) -> anyhow::Result<bool>
    where
        D: std::io::Write + Send + 'static,
    {
        if !self.opts.no_lints {
            for lint in self.lints() {
                writeln!(diagnostic_output, "warning: {lint}")?;
            }
        }

        if !self.opts.dump_spec_phase1 && !self.opts.no_fixups {
            self.patches(vars)?;
        }

        if !self.opts.no_lints {
            for lint in self.lints2() {
                writeln!(diagnostic_output, "warning: {lint}")?;
            }
        }

        if self.opts.dump_spec_phase1 || self.opts.dump_spec_phase2 {
            writeln!(diagnostic_output, "{:#?}", self.left)?;
            writeln!(diagnostic_output, "{:#?}", self.right)?;
            writeln!(diagnostic_output, "{:#?}", self.opts)?;
            writeln!(diagnostic_output, "{:#?}", self.beginning)?;
            return Ok(true);
        }

        Ok(false)
    }
}

impl ComposedArgument {
    fn print<D>(
        &self,
        printer: &mut ScenarioPrinter,
        vars: &mut IdentifierGenerator,
        global: &mut WebsocatGlobalArgs,
        diagnostic_output: &mut D,
    ) -> anyhow::Result<()>
    where
        D: std::io::Write + Send + 'static,
    {
        match self {
            ComposedArgument::Simple(argv) => {
                let args = WebsocatArgs::parse_from(argv);
                global.hoover(&args)?;

                let mut invocation = WebsocatInvocation::from_args(args, diagnostic_output)?;
                if invocation.prepare(diagnostic_output, vars)? {
                    anyhow::bail!("In --compose mode only --dump-spec and --dump-spec-phase0 are supported, not phase1 or phase2")
                }

                invocation.print_scenario(vars, printer)?;
            }
            ComposedArgument::Parallel(vec) => {
                printer.print_line("parallel([{");
                for (i, x) in vec.iter().enumerate() {
                    if i != 0 {
                        printer.print_line("},{");
                    }
                    printer.increase_indent();
                    x.print(printer, vars, global, diagnostic_output)?;
                    printer.decrease_indent();
                }
                printer.print_line("}])");
            }
            ComposedArgument::Sequential(vec) => {
                printer.print_line("sequential([{");
                for (i, x) in vec.iter().enumerate() {
                    if i != 0 {
                        printer.print_line("},{");
                    }
                    printer.increase_indent();
                    x.print(printer, vars, global, diagnostic_output)?;
                    printer.decrease_indent();
                }
                printer.print_line("}])");
            }
            ComposedArgument::Race(vec) => {
                printer.print_line("race([{");
                for (i, x) in vec.iter().enumerate() {
                    if i != 0 {
                        printer.print_line("},{");
                    }
                    printer.increase_indent();
                    x.print(printer, vars, global, diagnostic_output)?;
                    printer.decrease_indent();
                }
                printer.print_line("}])");
            }
        }
        Ok(())
    }
}
