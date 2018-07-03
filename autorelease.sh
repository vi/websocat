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
CA="cargo +stable"

V=$(cat Cargo.toml | grep '^version' | grep -o '\".*\"' | tr -d '"' | cut -d. -f 1-2)

echo Version: $V

mkdir -p "$D"

r() {
    TF="$D"/websocat${S}_${TT}${E}
    if [ -e "$TF" ]; then
        echo $TF already exists
        return
    fi
    echo "$T -> $TF"
    $CA rustc --bin websocat $FE --release -j2 --target $T -- -C lto
    cp ./target/$T/release/websocat${E} "$TF"
    ${ST} "${TF}"
}



l1() {

T=x86_64-unknown-linux-gnu
TT=amd64-linux
ST=strip
CA='cargo +stable'
r

export PKG_CONFIG_ALLOW_CROSS=1
T=i686-unknown-linux-gnu
TT=i386-linux
r

T=arm-unknown-linux-gnueabi
TT=arm-linux
CA="cross"
r

}

l2() {

T=arm-linux-androideabi
TT=arm-android
CA="cross"
r

T=i686-linux-android
TT=i386-android
CA="cross"
r

T=i686-unknown-linux-musl
TT=i386-linux-static
r

T=x86_64-unknown-linux-musl
TT=amd64-linux-static
r

T=x86_64-linux-android
TT=amd64-android
CA="cross"
r

#T=i686-unknown-freebsd
#TT=i386-freebsd
#CA=cross
#S=strip
#r
#
#T=x86_64-unknown-freebsd
#TT=amd64-freebsd
#CA=cross
#S=strip
#r

r

}


b() {

T=i686-pc-windows-gnu
#CA="cross +stable"
CA="cargo +stable"
TT=win32
ST=i586-mingw32msvc-strip
E=.exe
r

FE=--features=ssl
T=x86_64-pc-windows-gnu
CA="cross"
TT=win64
ST=amd64-mingw32msvc-strip
E=.exe
r

T=x86_64-apple-darwin
CA="cargo +stable"
TT=mac
E=
ST=/mnt/src/git/osxcross/target/bin/x86_64-apple-darwin15-strip
r

}



all() {

S=""
FE=--features=ssl,workaround1,seqpacket
l1

S=""
FE=--features=ssl,workaround1,seqpacket,openssl-probe
l2

S=""
FE=--features=ssl
b


S="_nossl"
FE=--features=workaround1,seqpacket
T=arm-unknown-linux-musleabi
TT=arm-linux-static
ST=strip
r

S="_nossl"
FE=--features=workaround1,seqpacket
l1

FE=--features=workaround1,seqpacket
l2

FE=
b

exit 0

}

all

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
