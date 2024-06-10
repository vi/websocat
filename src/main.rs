use types::Task;

use types::Handle;
use utils::run_task;

pub mod types;
pub mod trivials;
pub mod misc;
pub mod copydata;


pub mod fluff;
pub mod tcp;

pub mod scenario;

pub mod debugfluff;
pub mod utils;


#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    //tracing_subscriber::fmt().json().with_max_level(tracing::Level::DEBUG).init();
    tracing_subscriber::fmt::init();
    
    let f = std::fs::read(std::env::args().nth(1).unwrap())?;
    let global_scenario = std::str::from_utf8(&f[..])?;
    
    let ctx = scenario::load_scenario(global_scenario)?;

    let task: Handle<Task> = ctx.execute()?;
    run_task(task).await;

    Ok(())
}

mod all_functions;
