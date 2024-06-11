use rhai::{Engine, FnPtr, FuncArgs, NativeCallContext, Variant, AST};
use std::sync::{Arc, Weak};
use tracing::error;

use crate::{
    types::{Handle, Task},
    utils::run_task,
};

pub struct Scenario {
    pub ast: AST,
    pub engine: Engine,
}

pub trait ScenarioAccess {
    fn callback<T: Variant + Clone>(&self, f: FnPtr, args: impl FuncArgs) -> anyhow::Result<T>;
    fn get_scenario(&self) -> anyhow::Result<Arc<Scenario>>;
}

pub fn load_scenario(s: &str) -> anyhow::Result<Arc<Scenario>> {
    let mut engine = Engine::RAW;

    crate::all_functions::register_functions(&mut engine);

    let ast = engine.compile(s)?;
    let mut scenario = Scenario { ast, engine };

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
    fn callback<T: Variant + Clone>(&self, f: FnPtr, args: impl FuncArgs) -> anyhow::Result<T> {
        let scenario: Weak<Scenario> = self.tag().unwrap().clone().try_cast().unwrap();

        if let Some(s) = scenario.upgrade() {
            s.callback(f, args)
        } else {
            anyhow::bail!("Scenario is already terminated")
        }
    }

    fn get_scenario(&self) -> anyhow::Result<Arc<Scenario>> {
        let scenario: Weak<Scenario> = self.tag().unwrap().clone().try_cast().unwrap();

        if let Some(s) = scenario.upgrade() {
            Ok(s)
        } else {
            anyhow::bail!("Scenario is already terminated")
        }
    }
}

impl ScenarioAccess for Arc<Scenario> {
    fn callback<T: Variant + Clone>(&self, f: FnPtr, args: impl FuncArgs) -> anyhow::Result<T> {
        let scenario = self;

        let ret = f.call(&self.engine, &scenario.ast, args);
        if let Err(ref e) = ret {
            error!("Error from scenario task: {e}");
        };
        Ok(ret?)
    }

    fn get_scenario(&self) -> anyhow::Result<Arc<Scenario>> {
        Ok(self.clone())
    }
}

pub async fn callback_and_continue(ctx: Arc<Scenario>, f: FnPtr, args: impl FuncArgs) {
    match ctx.callback::<Handle<Task>>(f, args) {
        Ok(h) => run_task(h).await,
        Err(e) => error!("Error from scenario task: {e}"),
    };
}
