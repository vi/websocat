use std::sync::Mutex;

use rhai::{Engine, AST, Variant, FnPtr, FuncArgs};
use tracing::error;

use crate::types::{Handle, Task, run_task};

static SINGLETON : Mutex<Option<&'static GlobalContext>> = Mutex::new(None);

struct GlobalContext {
    engine: Engine,
    ast: AST,
}


pub fn load_global_scenario(s: &str) -> anyhow::Result<()> {
    let mut singleton = SINGLETON.lock().unwrap();

    if ! matches!(*singleton, None) {
        anyhow::bail!("Global scenario is already loaded");
    }

    let mut engine = Engine::RAW;

    crate::all_functions::register_functions(&mut engine);

    let ast = engine.compile(s)?;

    *singleton = Some(Box::leak(Box::new(GlobalContext { engine, ast })));

    Ok(())
}

pub fn execute_global_scenario() -> anyhow::Result<Handle<Task>> {
    let context = {
        let l = SINGLETON.lock().unwrap();
        match *l {
            Some(x) => x,
            None => anyhow::bail!("Global scenario not loaded"),
        }
    };


    let task: Handle<Task> = context.engine.eval_ast(&context.ast)?;

    Ok(task)
}

pub fn callback<T : Variant + Clone>(f: FnPtr, args: impl FuncArgs) -> anyhow::Result<T> {
    let context = {
        let l = SINGLETON.lock().unwrap();
        match *l {
            Some(x) => x,
            None => anyhow::bail!("Global scenario not loaded"),
        }
    };

    let ret = f.call(&context.engine, &context.ast, args);
    if let Err(ref e) = ret {
        error!("Error from scenario task: {e}");
    };
    Ok(ret?)
}

pub async fn callback_and_continue(f: FnPtr, args: impl FuncArgs) {
    match callback::<Handle<Task>> (f, args) {
        Ok(h) => run_task(h).await,
        Err(e) => error!("Error from scenario task: {e}"),
    };
}
