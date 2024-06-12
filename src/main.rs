

pub mod scenario_executor {
    pub mod copydata;

    pub mod misc;
    pub mod trivials;
    pub mod types;
    pub mod fluff;
    pub mod tcp;
    pub mod scenario;
    pub mod debugfluff;
    pub mod utils;
    pub mod wsupgrade;
    pub mod wsframer;

    pub mod all_functions;
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    //tracing_subscriber::fmt().json().with_max_level(tracing::Level::DEBUG).init();
    tracing_subscriber::fmt::init();

    let f = std::fs::read(std::env::args().nth(1).unwrap())?;
    let global_scenario = std::str::from_utf8(&f[..])?;

    let ctx = scenario_executor::scenario::load_scenario(global_scenario)?;

    let task: scenario_executor::types::Handle<scenario_executor::types::Task> = ctx.execute()?;
    scenario_executor::utils::run_task(task).await;

    Ok(())
}

