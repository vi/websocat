# Build stage
FROM rust:1.80.1-alpine3.20 AS cargo-build

RUN apk add --no-cache musl-dev pkgconfig openssl-dev

WORKDIR /src/websocat
ENV RUSTFLAGS='-Ctarget-feature=-crt-static'

COPY Cargo.toml Cargo.toml
ARG CARGO_OPTS="--features=workaround1,seqpacket,prometheus_peer,prometheus/process,crypto_peer"

RUN mkdir src/ &&\
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release $CARGO_OPTS && \
    rm -f target/release/deps/websocat*

COPY src src
RUN cargo build --release $CARGO_OPTS && \
    strip target/release/websocat

# Final stage
FROM alpine:3.20

RUN apk add --no-cache libgcc

WORKDIR /
COPY --from=cargo-build /src/websocat/target/release/websocat /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/websocat"]
