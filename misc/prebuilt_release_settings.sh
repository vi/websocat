targetsettings() {
  EXTRA_CARGO_FLAGS="--features=vendored_openssl"
  case "$1" in
     i686-unknown-linux-musl)
          EXTRA_CARGO_FLAGS="--no-default-features --features=socketoptions"
     ;;
     arm-unknown-linux-musleabi)
          EXTRA_CARGO_FLAGS="--no-default-features --features=rustls,socketoptions"
     ;;
     loongarch64-unknown-linux-musl)
          #EXTRA_CARGO_FLAGS="--no-default-features --features=rustls,socketoptions"
     ;;
     wasm32-wasip1)
          SKIP=1
     ;;
  esac
}
