#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature="tokioconsole")] {
        console_subscriber::init();
    }
    #[cfg(not(feature="tokioconsole"))] {
        tracing_subscriber::fmt::init();
    }
    let argv = std::env::args_os();
    let stderr = std::io::stderr();
    let time_base = tokio::time::Instant::now();
    let registry = websocat::scenario_executor::types::Registry::default();
    websocat::websocat_main(argv, stderr, time_base, true, registry).await
}
