#!/bin/bash

if [ "$TRAVIS_OS_NAME" = "osx" ]; then
    echo "Not supported on Mac";
    exit 0
fi

set -ex

PATH=target/debug:$PATH

websocat -q -t ws-l:127.0.0.1:19923 mirror:&
MIR=$!

trap 'kill $MIR' EXIT
trap 'echo test failed' ERR


function ensurenonblock() {
    perl -we 'use Fcntl qw(F_GETFL O_NONBLOCK); open F, "<&=", 0; my $flags = fcntl(F, F_GETFL, 0); if ($flags & O_NONBLOCK) { exit 1; } else { exit 0; }'
}

sleep 1

ensurenonblock

C1=$(find /proc/$MIR/fd -type l -printf '\n' | wc -l)

{
    echo 123
    sleep 1
    echo ABC
} | websocat ws://127.0.0.1:19923 | {
    TS1=$(date +%s.%N)
    read A
    TS2=$(date +%s.%N)
    read B
    TS3=$(date +%s.%N)

    echo TS1=$TS1 A=$A TS2=$TS2 B=$B TS3=$TS3 > lol
}

perl -ne '
    use POSIX;
    if(m!TS1=(\S+) A=123 TS2=(\S+) B=ABC TS3=(\S+)!) {
        if ($2-$1 > 0.1) {
            print STDERR "Err 1\n";
            exit 1;
        }
        if ($3-$2 < 0.7 || $3-$2 > 1.4) {
            print STDERR "Err 2\n";
            exit 1;
        }
        print STDERR "Timing OK\n";
        POSIX::_exit 0;
    }
    END {
        exit 1;
    }
' lol

rm -f lol

ensurenonblock

C2=$(find /proc/$MIR/fd -type l -printf '\n' | wc -l)
test "$C1" -eq "$C2"

websocat -b literal:qwe ws://127.0.0.1:19923
websocat -b literal:qwe -u ws://127.0.0.1:19923
websocat -b literal:qwe -1uU ws://127.0.0.1:19923

C2=$(find /proc/$MIR/fd -type l -printf '\n' | wc -l)
test "$C1" -eq "$C2"


echo Test OK
