#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let argv = std::env::args_os();
    let stderr = std::io::stderr();
    let time_base = tokio::time::Instant::now();
    websocat::websocat_main(argv, stderr, time_base, true).await
}
