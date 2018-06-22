#!/bin/bash

set -e

if [ -z "$1" ]; then
    echo "Usage: autorelease.sh directory/"
    exit 1
fi
D="$1"
S=""
E=""
ST=strip
FE=""

V=$(cat Cargo.toml | grep '^version' | grep -o '\".*\"' | tr -d '"' | cut -d. -f 1-2)

echo Version: $V

mkdir -p "$D"

r() {
    cargo +stable rustc --bin websocat $FE --release -j2 --target $T -- -C lto
    TF="$D"/websocat${S}_${V}_${T}${E}
    cp ./target/$T/release/websocat${E} "$TF"
    ${ST} "${TF}"
}

set -x

FE=--features=ssl
T=x86_64-unknown-linux-gnu
r


S=_nossl
FE=

T=i686-unknown-linux-gnu
r

T=i686-unknown-linux-musl
r

T=arm-linux-androideabi
r

T=arm-unknown-linux-musleabi
r

ST=/mnt/src/git/osxcross/target/bin/x86_64-apple-darwin15-strip
T=x86_64-apple-darwin
r

FE=
ST=i586-mingw32msvc-strip
E=.exe
T=i686-pc-windows-gnu
r


FE=ssl
S=
ST=i586-mingw32msvc-strip
E=.exe
T=i686-pc-windows-gnu
r

ST=/mnt/src/git/osxcross/target/bin/x86_64-apple-darwin15-strip
T=x86_64-apple-darwin
E=
r

set +x
echo "Next steps: 1. create tag; 2. upload release; 3. upload to crates.io"
