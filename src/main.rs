#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let argv = std::env::args_os();
    websocat::websocat_main(argv).await
}
