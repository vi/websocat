<!-- Note: this file is auto-generated -->
{{#include endpoints_header.md}}
## AbstractConnect

Connect to the specified abstract-namespaced UNIX socket (Linux)

Prefixes:

* `abstract:`
* `abstract-connect:`
* `connect-abstract:`
* `abstract-c:`
* `c-abstract:`
* `abs:`

## AbstractListen

Listen UNIX socket on specified abstract path (Linux)

Prefixes:

* `abstract-listen:`
* `listen-abstract:`
* `abstract-l:`
* `l-abstract:`
* `l-abs:`
* `abs-l:`

## AbstractSeqpacketConnect

Connect to specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name

Prefixes:

* `seqpacket-abstract:`
* `seqpacket-abstract-connect:`
* `seqpacket-abstract-c:`
* `abstract-seqpacket:`
* `abstract-seqpacket-connect:`
* `abstract-seqpacket-c:`
* `abs-seqp:`
* `seqp-abs:`

## AbstractSeqpacketListen

Listen specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name

Prefixes:

* `seqpacket-abstract-listen:`
* `seqpacket-abstract-l:`
* `abstract-seqpacket-listen:`
* `abstract-seqpacket-l:`
* `abs-seqp-l:`
* `seqp-abs-l:`
* `l-abs-seqp:`
* `l-seqp-abs:`

## AppendFile

Append to specified file.

Prefixes:

* `appendfile:`

## AsyncFd

Use specified inherited file descriptor for reading and writing, assuming it supports `read(2)` and `writev(2)` and can be put in epoll (or analogue).

Trying to specify unexisting FD, especially low-numbered (e.g from 3 to 20) may lead to undefined behaviour.

Gives a StreamSocket. If you want a packet-oriented socket, use `defragment:chunks:async-fd:X` overlay chain.

Prefixes:

* `async-fd:`
* `open-fd:`

## Cmd

Execute given command line and use its stdin/stdout as a socket.

Prefixes:

* `cmd:`
* `sh-c:`

## DummyDatagrams

Datagram socket that ignores all incoming data and signals EOF immediately

Prefixes:

* `empty:`
* `null:`
* `dummy-datagrams:`
* `dummy:`

## DummyStream

Byte stream socket that ignores all incoming data and immediately EOF-s read attempts

Prefixes:

* `devnull:`
* `dummy-stream:`

## Exec

Execute given program as a subprocess and use its stdin/stdout as a socket.
Specify command line arguments using `--exec-args` command line option.

Prefixes:

* `exec:`

## Literal

Byte stream socket that produces specified content and ignores incoming data

Prefixes:

* `literal:`

## LiteralBase64

Byte stream socket that produces specified content (base64-encoded) and ignores incoming data

Prefixes:

* `literal-base64:`

## Mirror

Read data that is written to this endpoint.

Prefixes:

* `mirror:`

## MockStreamSocket

Byte stream socket for tests that can produce and consume (assert)
data according to special scenario supplied as an argument

Prefixes:

* `mock_stream_socket:`
* `mock-stream-socket:`
* `mss:`

## Random

Generate random bytes

Prefixes:

* `random:`

## ReadFile

Read specified file. Ignores writes.

Prefixes:

* `readfile:`

## RegistryDatagramConnect

Connect to a virtual intra-Websocat address using a datagram socket

This is different from `registry-send:` that it creates a temporary buffer and can use overlays. The buffer is like two `mirror:` endpoints.

Prefixes:

* `registry-datagram-connect:`
* `regdg-c:`

## RegistryDatagramListen

Listen for virtual intra-Websocat datagram connections at specified address.

Connections can be made with `registry-datagrams-connect:` or `registry-send:` endpoints.

Prefixes:

* `registry-datagram-listen:`
* `regdg-l:`

## RegistrySend

Send the {socket this endpoint is paired with} to a virtual intra-Websocat address

Prefixes:

* `registry-send:`
* `regsend:`

## RegistryStreamConnect

Connect to a virtual intra-Websocat address using a stream socket

This is different from `registry-send:` that it creates a temporary buffer and can use overlays. The buffer is like two `mirror:` endpoints.

Prefixes:

* `registry-stream-connect:`
* `regstr-c:`

## RegistryStreamListen

Listen for virtual intra-Websocat stream connections at specified address.

Connections can be made with `registry-stream-connect:` and `registry-send:` endpoints.

Prefixes:

* `registry-stream-listen:`
* `regstr-l:`

## SeqpacketConnect

Connect to specified UNIX SOCK_SEQPACKET socket by path

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket:`
* `seqpacket-connect:`
* `connect-seqpacket:`
* `seqpacket-c:`
* `c-seqpacket:`
* `seqp:`

## SeqpacketListen

Listen specified UNIX SOCK_SEQPACKET socket

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket-listen:`
* `listen-seqpacket:`
* `seqpacket-l:`
* `l-seqpacket:`
* `l-seqp:`
* `seqp-l:`

## SeqpacketListenFd

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process)

Prefixes:

* `seqpacket-listen-fd:`
* `listen-seqpacket-fd:`
* `seqpacket-l-fd:`
* `l-seqpacket-fd:`
* `l-seqp-fd:`
* `seqp-l-fd:`

## SeqpacketListenFdNamed

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `seqpacket-listen-fdname:`
* `listen-seqpacket-fdname:`
* `seqpacket-l-fdname:`
* `l-seqpacket-fdname:`
* `l-seqp-fdname:`
* `seqp-l-fdname:`

## SimpleReuserEndpoint

Implementation detail of `reuse-raw:` overlay

This endpoint cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

## Stdio

Console, terminal: read bytes from stdin, write bytes to stdout.

Uses additional thread, which may cause lower latency and throughput.

Prefixes:

* `stdio:`

## TcpConnectByEarlyHostname


Connect to a TCP socket by hostname.
Hostname resolution happens once, on scenario start.
If multiple address are resolved, they are tried simultaneously, first connected one wins.

See prefixes for `TcpConnectByIp`.

## TcpConnectByIp

Connected to a TCP socket using one explicitly specified IPv4 or IPv6 socket address.

Prefixes:

* `tcp:`
* `tcp-connect:`
* `connect-tcp:`
* `tcp-c:`
* `c-tcp:`

## TcpConnectByLateHostname


Connect to a TCP socket by hostname.
Hostname resolution is repeated every time a connection is initiated.
If multiple address are resolved, they are tried simultaneously, first connected one wins.

See prefixes for `TcpConnectByIp`

## TcpListen

Listen for incoming TCP connections on one TCP socket, bound to the specified IPv4 or IPv6 address.

Prefixes:

* `tcp-listen:`
* `listen-tcp:`
* `tcp-l:`
* `l-tcp:`

## TcpListenFd

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process)

Prefixes:

* `tcp-listen-fd:`
* `listen-tcp-fd:`
* `tcp-l-fd:`
* `l-tcp-fd:`

## TcpListenFdNamed

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `tcp-listen-fdname:`
* `listen-tcp-fdname:`
* `tcp-l-fdname:`
* `l-tcp-fdname:`

## Tee

Implementation detail of `tee:` overlay

This endpoint cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

## UdpBind

Bind UDP socket to this address.
Command line options greatly affect behaviour of this endpoint. It can be turned into a flexible `UdpConnect` analogue.

Prefixes:

* `udp-bind:`
* `bind-udp:`
* `udp-listen:`
* `listen-udp:`
* `udp-l:`
* `l-udp:`

## UdpConnect

Connect to this UDP socket. Not affected by `--udp-bind-*`` CLI options.

Prefixes:

* `udp:`
* `udp-connect:`
* `connect-udp:`
* `udp-c:`
* `c-udp:`

## UdpFd

Use inherited pre-bound UDP socket from specified file descriptor.

Prefixes:

* `udp-fd:`
* `udp-bind-fd:`

## UdpFdNamed

Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES)

Prefixes:

* `udp-fdname:`
* `udp-bind-fdname:`

## UdpServer

Bind UDP socket and spawn a separate task for each client.
Connections get closed when there are too many active clients or by a timeout.

Prefixes:

* `udp-server:`

## UdpServerFd

Use inherited pre-bound UDP socket from specified file descriptor, spawning a task for each client

Prefixes:

* `udp-server-fd:`

## UdpServerFdNamed

Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES), spawning a task for each client

Prefixes:

* `udp-server-fdname:`

## UnixConnect

Connect to the specified UNIX socket path using stream socket

Prefixes:

* `unix:`
* `unix-connect:`
* `connect-unix:`
* `unix-c:`
* `c-unix:`

## UnixListen

Listen specified UNIX socket path for SOCK_STREAM connections

Prefixes:

* `unix-listen:`
* `listen-unix:`
* `unix-l:`
* `l-unix:`

## UnixListenFd

Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process)

Prefixes:

* `unix-listen-fd:`
* `listen-unix-fd:`
* `unix-l-fd:`
* `l-unix-fd:`

## UnixListenFdNamed

Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming connections,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `unix-listen-fdname:`
* `listen-unix-fdname:`
* `unix-l-fdname:`
* `l-unix-fdname:`

## WriteFile

Write specified file.

Prefixes:

* `writefile:`

## WriteSplitoff

Implementation detail of `write-splitoff:` overlay

This endpoint cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

## WsListen

Listen for incoming WebSocket connections at specified TCP socket address.

Prefixes:

* `ws-listen:`
* `ws-l:`
* `l-ws:`
* `listen-ws:`

## WsUrl

Connect to specified WebSocket plain (insecure) URL

Prefixes:

* `ws://`

## WssUrl

Connect to specified WebSocket TLS URL

Prefixes:

* `wss://`

## Zero

Generate zero bytes

Prefixes:

* `zero:`

