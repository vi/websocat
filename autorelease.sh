#!/bin/bash

set -e

if [ -z "$1" ]; then
    echo "Usage: autorelease.sh directory/"
    echo "This script is supposed to run only on my system"
    exit 1
fi
D="$1"


S=""
E=""
ST=strip
FE=""
CA="cargo +stable"

V=$(cat Cargo.toml | grep '^version' | grep -o '\".*\"' | tr -d '"' | cut -d. -f 1-3)

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

#T=x86_64-linux-android
#TT=amd64-android
#CA="cross"
#r

T=i686-unknown-freebsd
TT=i386-freebsd
CA=cross
ST=strip
r
#
T=x86_64-unknown-freebsd
TT=amd64-freebsd
CA=cross
ST=strip
r

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

T=x86_64-pc-windows-gnu
CA="cross"
TT=win64
ST=amd64-mingw32msvc-strip
E=.exe
r

PATH=$PATH:/mnt/src/git/osxcross/target/bin
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


F=$D/websocat_${V}_ssl1.1_amd64.deb
if [ ! -e "$F" ]; then
    cargo +stable deb --target=x86_64-unknown-linux-gnu
    cp -v target/x86_64-unknown-linux-gnu/debian/websocat_${V}_amd64.deb "$F"
    debian.vi-server.org-add "$F"
else
    echo "$F already exists"
fi

F=$D/websocat_${V}_ssl1.1_i386.deb
if [ ! -e "$F" ]; then
    cargo +stable deb --target=i686-unknown-linux-gnu
    cp -v target/i686-unknown-linux-gnu/debian/websocat_${V}_i386.deb "$F"
    debian.vi-server.org-add "$F"
else
    echo "$F already exists"
fi

F=$D/websocat_${V}_ssl1.0_amd64.deb
if [ ! -e "$F" ]; then
    trap 'mv Cargo.toml.bak Cargo.toml; rm -Rf target/ && mv target_ target' EXIT
    mv target target_
    ln -s /tmp/qqq target
    cp Cargo.toml Cargo.toml.bak
    cat Cargo.toml.bak | sed 's!libssl1.1!libssl1.0.0!' > Cargo.toml
    docker run --rm -it -v $PWD:/wd --entrypoint /bin/bash ubu1604rust -c 'source /root/.profile && mkdir /tmp/qqq && cd /wd && cargo deb --target=x86_64-unknown-linux-gnu && PKG_CONFIG_ALLOW_CROSS=1 cargo deb --target=i686-unknown-linux-gnu && cp target/*/debian/*.deb /wd/'
    cp -v websocat_${V}_amd64.deb "$F"
    cp -v websocat_${V}_i386.deb "${F/amd64/i386}"
else
    echo "ssl1.0 files already exist"
fi

set +x
echo "Next steps: 1. create tag; 2. upload release; 3. upload to crates.io; 4. debian.vi-server.org-upload"

exit 0

}

all


