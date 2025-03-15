targetsettings() {
  EXTRA_CARGO_FLAGS="--features=vendored_openssl"
  case "$1" in
     i686-unknown-linux-musl)
          EXTRA_CARGO_FLAGS="--no-default-features"
     ;;
  esac
}
