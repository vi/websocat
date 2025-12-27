targetsettings() {
  EXTRA_CARGO_FLAGS="--features=vendored_openssl,openssl-probe"
  case "$1" in
     i686-unknown-linux-musl)
          EXTRA_CARGO_FLAGS="--no-default-features --features=signal_handler,tokio-process,unix_stdio,compression"
     ;;
     arm-unknown-linux-musleabi)
          EXTRA_CARGO_FLAGS="--no-default-features --features=signal_handler,tokio-process,unix_stdio,compression"
     ;;
     loongarch64-unknown-linux-musl)
          EXTRA_CARGO_FLAGS="--no-default-features --features=signal_handler,tokio-process,unix_stdio,compression"
     ;;
     riscv64gc-unknown-linux-musl)
          EXTRA_CARGO_FLAGS="--no-default-features --features=signal_handler,tokio-process,unix_stdio,compression"
     ;;
     wasm32-wasip1)
          SKIP=1
     ;;
  esac
}
