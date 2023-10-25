use std::sync::{Arc, Mutex};

use rhai::{Engine, AST};
use types::Task;

use types::Handle;

pub mod types;
pub mod trivials;
pub mod misc;
pub mod copydata;


pub mod fluff;
pub mod tcp;


pub static THE_ENGINE : Mutex<Option<Arc<Engine>>> = Mutex::new(None);
pub static THE_AST : Mutex<Option<Arc<AST>>> = Mutex::new(None);



#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let f = std::fs::read(std::env::args().nth(1).unwrap())?;

    let mut engine = Engine::RAW;
    let ast = engine.compile(std::str::from_utf8(&f[..])?)?;

    //let engine_h : Handle<Engine> = Arc::new(Mutex::new(None));

    trivials::register(&mut engine);
    copydata::register(&mut engine);
    misc::register(&mut engine);
    tcp::register(&mut engine);
    fluff::register(&mut engine);
    

    let engine = Arc::new(engine);
    let ast = Arc::new(ast);
    *THE_ENGINE.lock().unwrap() = Some(engine.clone());
    *THE_AST.lock().unwrap() = Some(ast.clone());
    

    let task: Handle<Task> = engine.eval_ast(&ast)?;

    if let Some(t) = task.lock().unwrap().take() {
        t.await;
    } else {
        eprintln!("No task requested");
    }

    Ok(())
}
