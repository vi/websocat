use types::Task;

use types::Handle;
use types::run_task;

pub mod types;
pub mod trivials;
pub mod misc;
pub mod copydata;


pub mod fluff;
pub mod tcp;

pub mod scenario;





#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let f = std::fs::read(std::env::args().nth(1).unwrap())?;
    let global_scenario = std::str::from_utf8(&f[..])?;
    
    scenario::load_global_scenario(global_scenario)?;

    let task: Handle<Task> = scenario::execute_global_scenario()?;
    run_task(task).await;

    Ok(())
}

mod all_functions;
