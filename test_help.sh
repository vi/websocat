#!/bin/bash

# This test tracks how much of Websocat functionality is implemented by inspecting the help message

true ${WEBSOCAT:=target/debug/websocat}

T=$($WEBSOCAT --help=full)

CTR=0
SUCC=0

tt() {
   PAT="$1"
   true $((CTR+=1))
   if echo "$T" | cut -c 1-50 | tr ' \t' '\n\n' | grep -q -- "^$PAT\$"; then
      true $((SUCC+=1))
      printf '%40s  [ OK ]\n' "$PAT"
   else
      printf '%40s  [FAIL]\n' "$PAT"
   fi
}

tt --async-stdio
tt --dump-spec
tt --set-environment
tt --exit-on-eof
tt --foreachmsg-wait-read
tt --jsonrpc
tt --just-generate-key
tt --linemode-strip-newlines
tt --null-terminated
tt --no-line
tt --no-exit-on-zeromsg
tt --no-fixups
tt --no-async-stdio
tt --one-message
tt --oneshot
tt --exec-sighup-on-stdin-close
tt --exec-sighup-on-zero-msg
tt --reuser-send-zero-msg-on-disconnect
tt --server-mode
tt --strict
tt --insecure
tt --udp-broadcast
tt --udp-multicast-loop
tt --udp-oneshot
tt --udp-reuseaddr
tt --unidirectional
tt --unidirectional-reverse
tt --accept-from-fd
tt --unlink
tt --version
tt -v
tt --binary
tt --no-close
tt --websocket-ignore-zeromsg
tt --text
tt --base64
tt --base64-text
tt --socks5
tt --autoreconnect-delay-millis
tt --basic-auth
tt --queue-len
tt --buffer-size
tt --header
tt --server-header
tt --exec-args
tt --header-to-env
tt --help
tt --just-generate-accept
tt --max-messages
tt --max-messages-rev
tt --conncap
tt --origin
tt --pkcs12-der
tt --pkcs12-passwd
tt --request-header
tt --request-method
tt --request-uri
tt --restrict-uri
tt --static-file
tt --socks5-bind-script
tt --socks5-destination
tt --tls-domain
tt --udp-multicast
tt --udp-multicast-iface-v4
tt --udp-multicast-iface-v6
tt --udp-ttl
tt --protocol
tt --server-protocol
tt --websocket-version
tt --binary-prefix
tt --ws-c-uri
tt --ping-interval
tt --ping-timeout
tt --text-prefix
tt ws-listen:
tt inetd-ws:
tt l-ws-unix:
tt l-ws-abstract:
tt ws-lowlevel-client:
tt ws-lowlevel-server:
tt wss-listen:
tt http:
tt asyncstdio:
tt inetd:
tt tcp:
tt tcp-listen:
tt ssl-listen:
tt sh-c:
tt cmd:
tt exec:
tt readfile:
tt writefile:
tt appendfile:
tt udp:
tt udp-listen:
tt open-async:
tt open-fd:
tt threadedstdio:
tt unix:
tt unix-listen:
tt unix-dgram:
tt abstract:
tt abstract-listen:
tt abstract-dgram:
tt mirror:
tt literalreply:
tt clogged:
tt literal:
tt assert:
tt assert2:
tt seqpacket:
tt seqpacket-listen:
tt ws-upgrade:
tt http-request:
tt http-post-sse:
tt ssl-connect:
tt ssl-accept:
tt reuse-raw:
tt broadcast:
tt autoreconnect:
tt ws-c:
tt msg2line:
tt line2msg:
tt foreachmsg:
tt log:
tt jsonrpc:
tt socks5-connect:
tt socks5-bind:

echo "$SUCC of $CTR"

