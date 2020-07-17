# Build stage
FROM rust:1.45.0 as cargo-build

RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /src/websocat

COPY Cargo.toml Cargo.toml

RUN mkdir src/ &&\
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    RUSTFLAGS=-Clinker=musl-gcc cargo build --release --target=x86_64-unknown-linux-musl && \
    rm -f target/x86_64-unknown-linux-musl/release/deps/websocat*

COPY . .
RUN RUSTFLAGS=-Clinker=musl-gcc cargo build --release --target=x86_64-unknown-linux-musl

# Final stage
FROM alpine:3.12.0

WORKDIR /
COPY --from=cargo-build /src/websocat/target/x86_64-unknown-linux-musl/release/websocat /usr/local/bin

ENTRYPOINT ["/usr/local/bin/websocat"]
