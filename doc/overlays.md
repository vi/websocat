<!-- Note: this file is auto-generated -->
{{#include overlays_header.md}}
## LengthPrefixedChunks

Convert downstream stream-oriented socket to packet-oriented socket by prefixing each message with its length
(and maybe other flags, depending on options).

Prefixes:

* `lengthprefixed:`

## LineChunks

Convert downstream stream-oriented socket to packet-oriented socket by using newline byte as a packet separator.
Written data get modified to ensure that one line = one message.

May be automatically inserted in text (`-t`) mode.

Prefixes:

* `lines:`

## Log

Print encountered data to stderr for debugging

Prefixes:

* `log:`

## ReadChunkLimiter

Limit this stream's read buffer size to --read-buffer-limit
By splitting reads to many (e.g. single byte) chunks, we can
test and debug trickier code paths in various overlays

Prefixes:

* `read_chunk_limiter:`

## SimpleReuser

Share underlying datagram connection between multiple outer users.

All users can write messages to the socket, messages would be interleaved
(though each individual message should be atomic).
Messages coming from inner socket will be delivered to some one arbitrary connected user.
If that users disconnect, they will route to some other user.
A message can be lost when user disconnects.
User disconnections while writing a message may abort the whole reuser
(or result in a broken, trimmed message, depending on settings).

Prefixes:

* `reuse-raw:`
* `raw-reuse:`

## StreamChunks

Converts downstream stream-oriented socket to packet-oriented socket by chunking the stream arbitrarily (i.e. as syscalls happened to deliver the data)

May be automatically inserted in binary (`-b`) mode.

Prefixes:

* `chunks:`

## TlsClient

Establishes client-side TLS connection using specified stream-oriented downstream connection

Prefixes:

* `tls:`
* `ssl-connect:`
* `ssl-c:`
* `ssl:`
* `tls-connect:`
* `tls-c:`
* `c-ssl:`
* `connect-ssl:`
* `c-tls:`
* `connect-tls:`

## WriteBuffer

Insert write buffering layer that combines multiple write calls to one bigger

Prefixes:

* `write_buffer:`

## WriteChunkLimiter

Limit this stream's write buffer size to --write-buffer-limit
By enforcing short writes, we can
test and debug trickier code paths in various overlays

Prefixes:

* `write_chunk_limiter:`

## WriteSplitoff

Only read from inner specifier, route writes to other, CLI-specified Socket

Prefixes:

* `write-splitoff:`
* `write-divert:`
* `wrdvrt:`

## WsAccept

Expects a HTTP/1 WebSocket upgrade request from downstream stream socket. If valid, replies with Upgrade HTTP reply.
After than connects (i.e. exchanges bytes) downstream to upstream.

Does not provide WebSocket framing.

Prefixes:

* `ws-accept:`

## WsClient

Combined WebSocket upgrader and framer, but without TCP or TLS things
URI is taken from --ws-c-uri CLI argument
If it is not specified, it defaults to `/`, with a missing `host:` header

Prefixes:

* `ws-connect:`
* `connect-ws:`
* `ws-c:`
* `c-ws:`

## WsFramer

Converts downstream stream to upstream packets using WebSocket framing.

Automatically handles WebSocket pings and CloseFrames, but does not fully terminate the connection on CloseFrame, only signaling EOF instead.

Client or server mode is chosen depending on prefix you use.

Prefixes:

* `ws-lowlevel-client:`
* `ws-ll-client:`
* `ws-ll-c:`
* `ws-lowlevel-server:`
* `ws-ll-server:`
* `ws-ll-s:`

## WsServer

Combined WebSocket acceptor and framer.

Prefixes:

* `ws-upgrade:`
* `upgrade-ws:`
* `ws-u:`
* `u-ws:`

## WsUpgrade

Send HTTP/1 WebSocket upgrade to specified stream-oriented connection and accept and parse a reply,
then connects (i.e. exchanges bytes) the downstream connection to upstream.

Does not provide WebSocket framing.

Prefixes:

* `ws-request:`
* `ws-r:`

