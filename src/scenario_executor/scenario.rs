use rhai::{Engine, EvalAltResult, FnPtr, FuncArgs, NativeCallContext, Variant, AST};
use std::sync::{
    Arc, Mutex, Weak,
};
use tokio::time::Instant;
use tracing::{error};

use crate::scenario_executor::{
    exit_code::ExitCodeTracker, types::{Handle, Task}, utils1::run_task
};

use super::{
    types::{DiagnosticOutput, RandomnessSource, Registry},
    utils1::RhResult,
};

pub struct Scenario {
    pub ast: AST,
    pub engine: Engine,
    pub diagnostic_output: Mutex<DiagnosticOutput>,
    pub time_base: Instant,
    pub prng: Mutex<RandomnessSource>,
    pub registry: Registry,
    pub exit_code: ExitCodeTracker,
}

pub trait ScenarioAccess {
    fn callback<T: Variant + Clone, A: FuncArgs>(&self, f: FnPtr, args: A) -> anyhow::Result<T>;
    fn get_scenario(&self) -> RhResult<Arc<Scenario>>;
}

pub fn load_scenario(
    s: &str,
    diagnostic_output: DiagnosticOutput,
    time_base: Instant,
    prng: RandomnessSource,
    registry: Registry,
    exit_code: ExitCodeTracker,
) -> anyhow::Result<Arc<Scenario>> {
    let mut engine = Engine::RAW;

    crate::scenario_executor::all_functions::register_types(&mut engine);
    crate::scenario_executor::all_functions::register_functions(&mut engine);

    engine.set_max_expr_depths(1000, 1000);

    let ast = engine.compile(s)?;
    let mut scenario = Scenario {
        ast,
        engine,
        diagnostic_output: Mutex::new(diagnostic_output),
        time_base,
        prng: Mutex::new(prng),
        registry,
        exit_code,
    };

    let scenario_arc: Arc<Scenario> = Arc::new_cyclic(move |weak_scenario_arc| {
        let weak_scenario_arc: Weak<Scenario> = weak_scenario_arc.clone();
        scenario
            .engine
            .set_default_tag(rhai::Dynamic::from(weak_scenario_arc));
        scenario
    });

    Ok(scenario_arc)
}

impl Scenario {
    pub fn execute(&self) -> anyhow::Result<Handle<Task>> {
        let task: Handle<Task> = self.engine.eval_ast(&self.ast)?;
        Ok(task)
    }
}

impl ScenarioAccess for NativeCallContext<'_> {
    fn callback<T: Variant + Clone, A: FuncArgs>(&self, f: FnPtr, args: A) -> anyhow::Result<T> {
        let scenario: Weak<Scenario> = self.tag().unwrap().clone().try_cast().unwrap();

        if let Some(s) = scenario.upgrade() {
            s.callback(f, args)
        } else {
            anyhow::bail!("Scenario is already terminated")
        }
    }

    fn get_scenario(&self) -> RhResult<Arc<Scenario>> {
        let scenario: Weak<Scenario> = self.tag().unwrap().clone().try_cast().unwrap();

        if let Some(s) = scenario.upgrade() {
            Ok(s)
        } else {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                rhai::Dynamic::from(
                    "Scenario is already terminating, cannot make a callback into it",
                ),
                rhai::Position::NONE,
            )))
        }
    }
}

impl ScenarioAccess for Arc<Scenario> {
    fn callback<T: Variant + Clone, A: FuncArgs>(&self, f: FnPtr, args: A) -> anyhow::Result<T> {
        let scenario = self;

        let ret = f.call(&self.engine, &scenario.ast, args);
        if let Err(ref e) = ret {
            error!("Error from scenario task: {e}");
        };
        Ok(ret?)
    }

    fn get_scenario(&self) -> RhResult<Arc<Scenario>> {
        Ok(self.clone())
    }
}

pub async fn callback_and_continue<A: FuncArgs>(ctx: Arc<Scenario>, f: FnPtr, args: A) {
    match ctx.callback::<Handle<Task>, A>(f, args) {
        Ok(h) => run_task(h).await,
        Err(e) => error!("Error from scenario task: {e}"),
    };
}
