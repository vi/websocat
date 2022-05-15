# Build stage
FROM rust:1.60-alpine3.15 as cargo-build

RUN apk add --no-cache musl-dev pkgconfig openssl-dev

WORKDIR /src/websocat

COPY Cargo.toml Cargo.toml
ARG CARGO_OPTS="--features=workaround1,seqpacket,prometheus_peer,prometheus/process,crypto_peer"

RUN mkdir src/ &&\
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release --target=x86_64-unknown-linux-musl $CARGO_OPTS && \
    rm -f target/x86_64-unknown-linux-musl/release/deps/websocat*

COPY src src
RUN cargo build --release --target=x86_64-unknown-linux-musl $CARGO_OPTS && \
    strip target/x86_64-unknown-linux-musl/release/websocat

# Final stage
FROM alpine:3.15

WORKDIR /
COPY --from=cargo-build /src/websocat/target/x86_64-unknown-linux-musl/release/websocat /usr/local/bin

ENTRYPOINT ["/usr/local/bin/websocat"]
