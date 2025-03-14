# Command-line interface

This section describes options, flags and specifiers of Websocat CLI.

## `--help` output

```
Command-line client for web sockets, like netcat/curl/socat for ws://.

Usage: websocat [OPTIONS] <SPEC1> [SPEC2]

Arguments:
  <SPEC1>
          Left endpoint (e.g. a WebSocket URL). May be prefixed by one or more overlays

  [SPEC2]
          Right endpoint (or stdout if omitted). May be prefixed by one or more overlays

Options:
      --dump-spec
          do not execute this Websocat invocation, print equivalent Rhai script instead

      --dump-spec-phase0
          do not execute this Websocat invocation, print debug representation of specified arguments. In --compose mode it should be the second argument (i.e. just after --compose)

      --dump-spec-phase1
          do not execute this Websocat invocation, print debug representation of specified arguments

      --dump-spec-phase2
          do not execute this Websocat invocation, print debug representation of specified arguments

  -x, --scenario
          execute specified file as Rhai script (e.g. resutling from --dump-spec option output)

  -t, --text
          use text mode (one line = one WebSocket text message)

  -b, --binary
          use binary mode (arbitrary byte chunk = one WebSocket binary message)

      --late-resolve
          resolve hostnames to IP addresses late (every time when forwarding a connection) instead of one time at the beginning

  -k, --insecure
          accept invalid domains and root certificates for TLS client connections

      --tls-domain <TLS_DOMAIN>
          manually specify domain for `tls:` overlay or override domain for `wss://` URLs

  -s, --server
          listen for WebSocket conenctions instead of establishing client WebSocket connection

      --log-verbose
          log more data from `log:` overlay

      --log-omit-content
          do not log full content of the data from `log:` overlay, just chunk lengths

      --log-hex
          use hex lines instead of escaped characters for `log:`` overlay

      --log-timestamps
          Include relative timestamps in log messages

      --log-traffic
          automatically insert `log:` overlay in an apprioriate place to debug issues by displaying traffic chunks

      --ws-c-uri <WS_C_URI>
          URI for `ws-c:` overlay

      --read-buffer-limit <READ_BUFFER_LIMIT>
          paramemter for read_chunk_limiter: overlay, defaults to 1

      --write-buffer-limit <WRITE_BUFFER_LIMIT>
          paramemter for write_chunk_limiter: overlay, defaults to 1

      --separator <SEPARATOR>
          override byte value that separates stdin-supplied text WebSocket messages from each other from default '\n'

      --separator-n <SEPARATOR_N>
          require this number of newline (or other) bytes to separate WebSocket messages

      --separator-inhibit-substitution
          prevent mangling incoming text WebSocket by replacing `\n`  (or other separator sequence) with spaces (and trimming leading and trailing separator bytes)

      --udp-bind-target-addr <UDP_BIND_TARGET_ADDR>
          initial target sendto address for `udp-bind:` mode. If unset, it will try to send to neutral address (unsuccessfully)

      --udp-bind-restrict-to-one-address
          only allow incoming datagrams from specified target address for `upd-bind:` mode

      --udp-bind-redirect-to-last-seen-address
          automatically change target address for `udp-bind:` mode based in coming datagrams

      --udp-bind-connect-to-first-seen-address
          turn `udp-bind:` into `udp-connect:` as soon as we receive some datagram. Implied when `--udp-bind-target-addr` is not specified

      --udp-bind-inhibit-send-errors
          ignore failed `sendto` calls. Attempts to send without a configured target address are ignored implicitly

      --udp-server-timeout-ms <UDP_SERVER_TIMEOUT_MS>
          Client timeout of udp-server: mode

      --udp-server-max-clients <UDP_SERVER_MAX_CLIENTS>
          Maximum number of parallel handlers in udp-server: mode

      --udp-server-buffer-size <UDP_SERVER_BUFFER_SIZE>
          Size of receive buffer for udp-server: mode. `-B` is distinct, but can also affect operation

      --udp-server-qlen <UDP_SERVER_QLEN>
          Queue length for udp-server: mode

      --udp-server-backpressure
          Delay receiving more datagrams in udp-server: mode instead of dropping them in case of slow handlers

      --exec-args [<EXEC_ARGS>...]
          Command line arguments for `exec:` endpoint.
          
          This option is interpreted specially: it stops processing all other options uses everything after it as a part of the command line

      --exec-monitor-exits
          Immediately expire `cmd:` or `exec:` endpoints if child process terminates.
          
          This may discard some data that remained buffered in a pipe.

      --exec-uid <EXEC_UID>
          On Unix, try to set uid to this numeric value for the subprocess

      --exec-gid <EXEC_GID>
          On Unix, try to set uid to this numeric value for the subprocess

      --exec-chdir <EXEC_CHDIR>
          Try to change current directory to this value for the subprocess

      --exec-windows-creation-flags <EXEC_WINDOWS_CREATION_FLAGS>
          On Windows, try to set this numeric process creation flags

      --exec-arg0 <EXEC_ARG0>
          On Unix, set first subprocess's argv[0] to this value

      --exec-dup2 <EXEC_DUP2>
          On Unix, use dup2 and forward sockets directly to child processes (ignoring any overlays) instead of piping though stdin/stdout. Argument is comma-separated list of file descriptor slots to duplicate the socket into, e.g. `0,1` for stdin and stdout

      --exec-dup2-keep-nonblocking
          When using --exec-dup2, do not set inherited file descriptors to blocking mode

      --exec-dup2-execve
          on Unix, When using `--exec-dup2`, do not return to Websocat, instead substitude Websocat process with the given command

      --dummy-hangup
          Make dummy nodes also immediately signal hangup

      --exit-on-hangup
          Exit the whole process if hangup is detected

      --exit-after-one-session
          Exit the whole process after serving one connection; alternative to to --oneshot

  -u, --unidirectional
          Transfer data only from left to right specifier

  -U, --unidirectional-reverse
          Transfer data only from right to left specifier

      --unidirectional-late-drop
          Do not shutdown inactive directions when using `-u` or `-U`

  -E, --exit-on-eof
          Stop transferring data when one of the transfer directions reached EOF

  -B, --buffer-size <BUFFER_SIZE>
          Override buffer size for main data transfer session. Note that some overlays and endpoints may have separate buffers with sepaparately adjustable sizes.
          
          Message can span multiple over multiple fragments and exceed this buffer size

  -n, --no-close
          Do not send WebSocket close message when there is no more data to send there

      --ws-no-flush
          Do not flush after each WebSocket frame

      --ws-shutdown-socket-on-eof
          Shutdown write direction of the underlying socket backing a WebSocket on EOF

      --ws-ignore-invalid-masks
          Do not fail WebSocket connections if maksed frame arrives instead of unmasked or vice versa

      --ws-dont-check-headers
          Ignore absense or invalid values of `Sec-Websocket-*` things and just continue connecting

      --ws-no-auto-buffer
          Do not automatically insert buffering layer after WebSocket if underlying connections does not support `writev`

      --ws-omit-headers
          Skip request or response headers for Websocket upgrade

  -H, --header <HEADER>
          Add this custom header to WebSocket upgrade request when connecting to a Websocket. Colon separates name and value

      --server-header <SERVER_HEADER>
          Add this custom header to WebSocket upgrade response when serving a Websocket connection. Colon separates name and value

      --protocol <PROTOCOL>
          Specify this Sec-WebSocket-Protocol: header when connecting to a WebSocket

      --server-protocol <SERVER_PROTOCOL>
          Use this `Sec-WebSocket-Protocol:` value when serving a Websocket, and reject incoming connections if the don't specify this protocol

      --server-protocol-lax
          Don't reject incoming connections that fail to specify proper `Sec-WebSocket-Protocol` header. The header would be omitted from the response in this case

      --server-protocol-choose-first
          If client specifies Sec-WebSocket-Protocol, choose the first mentioned protocol and use if for response's Sec-WebSocket-Protocol

      --unlink
          When listening UNIX sockets, attempt to delete the file first to avoid the failure to bind

      --chmod-owner
          When listening UNIX sockets, change socket filesystem permissions to only allow owner connections

      --chmod-group
          When listening UNIX sockets, change socket filesystem permissions to allow owner and group connections

      --chmod-everyone
          When listening UNIX sockets, change socket filesystem permissions to allow connections from everywhere

      --oneshot
          Serve only one connection

      --no-lints
          Do not display warnings about potential CLI misusage

      --no-fixups
          Do not automatically transform endpoints and overlays to their appropriate low-level form. Many things will fail in this mode

      --udp-max-send-datagram-size <UDP_MAX_SEND_DATAGRAM_SIZE>
          Maximum size of an outgoing UDP datagram. Incoming datagram size is likely limited by --buffer-size
          
          [default: 4096]

      --seqpacket-max-send-datagram-size <SEQPACKET_MAX_SEND_DATAGRAM_SIZE>
          Maximum size of an outgoing SEQPACKET datagram. Incoming datagram size is likely limited by --buffer-size
          
          [default: 1048576]

      --random-seed <RANDOM_SEED>
          Use specified random seed instead of initialising RNG from OS

      --registry-connect-bufsize <REGISTRY_CONNECT_BUFSIZE>
          Use specified max buffer size for
          
          [default: 1024]

      --lengthprefixed-little-endian
          Use little-endian framing headers instead of big-endian for `lengthprefixed:` overlay

      --lengthprefixed-skip-read-direction
          Only affect one direction of the `lengthprefixed:` overlay, bypass tranformation for the other one

      --lengthprefixed-skip-write-direction
          Only affect one direction of the `lengthprefixed:` overlay, bypass tranformation for the other one

      --lengthprefixed-nbytes <LENGTHPREFIXED_NBYTES>
          Use this number of length header bytes for `lengthprefixed:` overlay
          
          [default: 4]

      --lengthprefixed-continuations
          Do not reassume message from fragments, stream them as chunks. Highest bit of the prefix would be set if the message is non-final

      --lengthprefixed-max-message-size <LENGTHPREFIXED_MAX_MESSAGE_SIZE>
          Maximum size of `lengthprefixed:` message (that needs to be reassembled from fragments)
          
          Connections would fail when messages exceed this size.
          
          Ignored if `--lengthprefixed-continuations` is active, but `nbytes`-based limitation can still fail connections.
          
          [default: 1048576]

      --lengthprefixed-include-control
          Include inline control messages (i.e. WebSocket pings or close frames) as content in `lengthprefixed:`.
          
          A bit would be set to signify a control message and opcode will be prepended as the first byte.
          
          When both continuations and contols are enabled, control messages may appear between continued data message chunks. Control messages can themselves be subject to continuations.

      --lengthprefixed-tag-text
          Set a bit in the prefix of `lengthprefixed:` frames when the frame denotes a text WebSocket message instead of binary

      --inhibit-pongs <INHIBIT_PONGS>
          Stop automatic replying to WebSocket pings after sending specified number of pongs. May be zero to just disable replying to pongs

      --global-timeout-ms <GLOBAL_TIMEOUT_MS>
          Abort whatever Websocat is doing after specified number of milliseconds, regardless of whether something is connected or not

      --global-timeout-force-exit
          Force process exit when global timeout is reached

      --sleep-ms-before-start <SLEEP_MS_BEFORE_START>
          Wait for this number of milliseconds before starting endpoints. Mostly indended for testing Websocat, in combination with --compose mode

      --stdout-announce-listening-ports
          Print a line to stdout when a port you requested to be listened is ready to accept connections

      --exec-after-listen <EXEC_AFTER_LISTEN>
          Execute this command line after binding listening port
          
          Connections are not actually accepted until the command exits (exit code is ignored)

      --exec-after-listen-append-port
          Append TCP or UDP port number to command line specified in --exec-after-listen
          
          Makes listening port "0" practical.

      --accept-from-fd
          Show dedicated error message explaining how to migrate Websocat1's --accpet-from-fd to the new scheme

      --reuser-tolerate-torn-msgs
          When using `reuse-raw:` (including automatically inserted), do not abort connections on unrecoverable broken messages

      --compose
          Interpret special command line arguments like `&`, `;`, '^', `(` and `)` as separators for composed scenarios mode. This argument must come first.
          
          This allows to execute multiple subscenarios in one Websocat invocation, sequentially (;), in parallel (&) or in parallel with early exit (^). You can also use parentheses to combine disparate operations.

      --write-file-no-overwrite
          For `writefile:` endpoint, do not overwrite existing files

      --write-file-auto-rename
          For `writefile:` endpoint, do not overwrite existing files, instead use other, neighbouring file names

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Short list of endpoint prefixes:
  abstract:
  abstract-listen:
  seqpacket-abstract:
  seqpacket-abstract-listen:
  appendfile:
  async-fd:
  cmd:
  empty:
  devnull:
  exec:
  literal:
  literal-base64:
  mock_stream_socket:
  readfile:
  registry-stream-connect:
  registry-stream-listen:
  seqpacket:
  seqpacket-listen:
  seqpacket-listen-fd:
  seqpacket-listen-fdname:
  stdio:
  tcp:
  tcp-listen:
  tcp-listen-fd:
  tcp-listen-fdname:
  udp-bind:
  udp:
  udp-fd:
  udp-fdname:
  udp-server:
  udp-server-fd:
  udp-server-fdname:
  unix:
  unix-listen:
  unix-listen-fd:
  unix-listen-fdname:
  writefile:
  ws-listen:
  ws://
  wss://

Short list of overlay prefixes:
  lengthprefixed:
  lines:
  log:
  read_chunk_limiter:
  reuse-raw:
  chunks:
  tls:
  write_buffer:
  write_chunk_limiter:
  ws-accept:
  ws-connect:
  ws-lowlevel-client:
  ws-lowlevel-server:
  ws-upgrade:
  ws-request:

Examples:

  websocat ws://127.0.0.1:1234
    Simple WebSocket client

  websocat -s 1234
    Simple WebSocket server

  websocat -b tcp-l:127.0.0.1:1234 wss://ws.vi-server.org/mirror
    TCP-to-WebSocket converter

  websocat -b ws-l:127.0.0.1:8080 udp:127.0.0.1:1234
    WebSocket-to-UDP converter

Use doc.md for reference of all Websocat functions
```


## Endpoints

### AbstractConnect

Connect to the specified abstract-namespaced UNIX socket (Linux)

Prefixes:

* `abstract:`
* `abstract-connect:`
* `connect-abstract:`
* `abstract-c:`
* `c-abstract:`
* `abs:`

### AbstractListen

Listen UNIX socket on specified abstract path (Linux)

Prefixes:

* `abstract-listen:`
* `listen-abstract:`
* `abstract-l:`
* `l-abstract:`
* `l-abs:`
* `abs-l:`

### AbstractSeqpacketConnect

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

### AbstractSeqpacketListen

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

### AppendFile

Append to specified file.

Prefixes:

* `appendfile:`

### AsyncFd

Use specified inherited file desciptor for reading and writing, assuming it supports `read(2)` and `writev(2)` and can be put in epoll (or analogue).

Trying to specify unexisting FD, especially low-numbered (e.g from 3 to 20) may lead to undefined behaviour.

Prefixes:

* `async-fd:`
* `open-fd:`

### Cmd

Execute given command line and use its stdin/stdout as a socket.

Prefixes:

* `cmd:`
* `sh-c:`

### DummyDatagrams

Datagram socket that ignores all incoming data and signals EOF immediately

Prefixes:

* `empty:`
* `null:`
* `dummy-datagrams:`
* `dummy:`

### DummyStream

Byte stream socket that ignores all incoming data and immediately EOF-s read attempts

Prefixes:

* `devnull:`
* `dummy-stream:`

### Exec

Execute given program as a subprocess and use its stdin/stdout as a socket.
Specify command line arguments using `--exec-args` command line option.

Prefixes:

* `exec:`

### Literal

Byte stream socket that produces specified content and ignores incoming data

Prefixes:

* `literal:`

### LiteralBase64

Byte stream socket that produces specified content (base64-encoded) and ignores incoming data

Prefixes:

* `literal-base64:`

### MockStreamSocket

Byte stream socket for tests that can produce and consume (assert)
data according to special scenario supplied as an argument

Prefixes:

* `mock_stream_socket:`
* `mock-stream-socket:`

### ReadFile

Read specified file. Ignores writes.

Prefixes:

* `readfile:`

### RegistryStreamConnect

Connect to a virtual intra-Websocat address using stream socket

Prefixes:

* `registry-stream-connect:`

### RegistryStreamListen

Listen for virtual intra-Websocat stream connections at specified address

Prefixes:

* `registry-stream-listen:`

### SeqpacketConnect

Connect to specified UNIX SOCK_SEQPACKET socket by path

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket:`
* `seqpacket-connect:`
* `connect-seqpacket:`
* `seqpacket-c:`
* `c-seqpacket:`
* `seqp:`

### SeqpacketListen

Listen specified UNIX SOCK_SEQPACKET socket

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket-listen:`
* `listen-seqpacket:`
* `seqpacket-l:`
* `l-seqpacket:`
* `l-seqp:`
* `seqp-l:`

### SeqpacketListenFd

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process)

Prefixes:

* `seqpacket-listen-fd:`
* `listen-seqpacket-fd:`
* `seqpacket-l-fd:`
* `l-seqpacket-fd:`
* `l-seqp-fd:`
* `seqp-l-fd:`

### SeqpacketListenFdNamed

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `seqpacket-listen-fdname:`
* `listen-seqpacket-fdname:`
* `seqpacket-l-fdname:`
* `l-seqpacket-fdname:`
* `l-seqp-fdname:`
* `seqp-l-fdname:`

### SimpleReuserEndpoint

Implementation detail of `reuse-raw:` overlay

This endpoint cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

### Stdio

Console, terminal: read bytes from stdin, write bytes to stdout.

Uses additional thread, which may cause lower latency and thoughput.

Prefixes:

* `stdio:`

### TcpConnectByEarlyHostname


Connect to a TCP socket by hostname.
Hostname resolution happens once, on scenario start.
If multiple address are resolved, they are tried simultaneously, first connected one wins.

See prefixes for `TcpConnectByIp`.

### TcpConnectByIp

Connected to a TCP socket using one explicitly specified IPv4 or IPv6 socket address.

Prefixes:

* `tcp:`
* `tcp-connect:`
* `connect-tcp:`
* `tcp-c:`
* `c-tcp:`

### TcpConnectByLateHostname


Connect to a TCP socket by hostname.
Hostname resolution is repeated every time a connection is initated.
If multiple address are resolved, they are tried simultaneously, first connected one wins.

See prefixes for `TcpConnectByIp`

### TcpListen

Listen for incoming TCP connections on one TCP socket, bound to the specified IPv4 or IPv6 address.

Prefixes:

* `tcp-listen:`
* `listen-tcp:`
* `tcp-l:`
* `l-tcp:`

### TcpListenFd

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process)

Prefixes:

* `tcp-listen-fd:`
* `listen-tcp-fd:`
* `tcp-l-fd:`
* `l-tcp-fd:`

### TcpListenFdNamed

Listen for incoming TCP connections on one TCP socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `tcp-listen-fdname:`
* `listen-tcp-fdname:`
* `tcp-l-fdname:`
* `l-tcp-fdname:`

### UdpBind

Bind UDP socket to this address.
Commmand line options greatly affect behaviour of this endpoint. It can be turned into a flexible `UdpConnect` analogue.

Prefixes:

* `udp-bind:`
* `bind-udp:`
* `udp-listen:`
* `listen-udp:`
* `udp-l:`
* `l-udp:`

### UdpConnect

Connect to this UDP socket. Not affected by `--udp-bind-*`` CLI options.

Prefixes:

* `udp:`
* `udp-connect:`
* `connect-udp:`
* `udp-c:`
* `c-udp:`

### UdpFd

Use inherited pre-bound UDP socket from specified file descriptor.

Prefixes:

* `udp-fd:`
* `udp-bind-fd:`

### UdpFdNamed

Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES)

Prefixes:

* `udp-fdname:`
* `udp-bind-fdname:`

### UdpServer

Bind UDP socket and spawn a separate task for each client.
Connections get closed when there are too many active clients or by a timeout.

Prefixes:

* `udp-server:`

### UdpServerFd

Use inherited pre-bound UDP socket from specified file descriptor, spawning a task for each client

Prefixes:

* `udp-server-fd:`

### UdpServerFdNamed

Use inherited pre-bound UDP socket from specified file descriptor (using LISTEN_FDNAMES), spawning a task for each client

Prefixes:

* `udp-server-fdname:`

### UnixConnect

Connect to the specified UNIX socket path using stream socket

Prefixes:

* `unix:`
* `unix-connect:`
* `connect-unix:`
* `unix-c:`
* `c-unix:`

### UnixListen

Listen specified UNIX socket path for SOCK_STREAM connections

Prefixes:

* `unix-listen:`
* `listen-unix:`
* `unix-l:`
* `l-unix:`

### UnixListenFd

Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process)

Prefixes:

* `unix-listen-fd:`
* `listen-unix-fd:`
* `unix-l-fd:`
* `l-unix-fd:`

### UnixListenFdNamed

Listen for incoming AF_UNIX SOCK_STREAM connections on one socket that is already ready for accepting incoming conenctions,
with specified file descriptor (inherited from parent process) based on LISTEN_FDNAMES environment variable (i.e. from SystemD)

Prefixes:

* `unix-listen-fdname:`
* `listen-unix-fdname:`
* `unix-l-fdname:`
* `l-unix-fdname:`

### WriteFile

Write specified file.

Prefixes:

* `writefile:`

### WsListen

Listen for incoming WebSocket connections at specified TCP socket address.

Prefixes:

* `ws-listen:`
* `ws-l:`
* `l-ws:`
* `listen-ws:`

### WsUrl

Connect to specified WebSocket plain (insecure) URL

Prefixes:

* `ws://`

### WssUrl

Connect to specified WebSocket TLS URL

Prefixes:

* `wss://`


## Overlays

### LengthPrefixedChunks

Convert downstream stream-oriended socket to packet-oriended socket by prefixing each message with its length
(and maybe other flags, depending on options).

Prefixes:

* `lengthprefixed:`

### LineChunks

Convert downstream stream-oriented socket to packet-oriented socket by using newline byte as a packet separator.
Written data get modified to ensure that one line = one message.

May be automatically inserted in text (`-t`) mode.

Prefixes:

* `lines:`

### Log

Print encountered data to stderr for debugging

Prefixes:

* `log:`

### ReadChunkLimiter

Limit this stream's read buffer size to --read-buffer-limit
By splitting reads to many (e.g. single byte) chunks, we can
test and debug trickier code paths in various overlays

Prefixes:

* `read_chunk_limiter:`

### SimpleReuser

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

### StreamChunks

Converts downstream stream-oriented socket to packet-oriented socket by chunking the stream arbitrarily (i.e. as syscalls happend to deliver the data)

May be automatically inserted in binary (`-b`) mode.

Prefixes:

* `chunks:`

### TlsClient

Establishes client-side TLS connection using specified stream-oriended downstream connection

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

### WriteBuffer

Insert write buffering layer that combines multiple write calls to one bigger

Prefixes:

* `write_buffer:`

### WriteChunkLimiter

Limit this stream's write buffer size to --write-buffer-limit
By enforcing short writes, we can
test and debug trickier code paths in various overlays

Prefixes:

* `write_chunk_limiter:`

### WsAccept

Expects a HTTP/1 WebSocket upgrade request from downstream stream socket. If valid, replies with Upgrade HTTP reply.
After than connects (i.e. exchanges bytes) downstream to upstream.

Does not provide WebSocket framing.

Prefixes:

* `ws-accept:`

### WsClient

Combined WebSocket upgrader and framer, but without TCP or TLS things
URI is taked from --ws-c-uri CLI argument
If it is not specified, it defaults to `/`, with a missing `host:` header

Prefixes:

* `ws-connect:`
* `connect-ws:`
* `ws-c:`
* `c-ws:`

### WsFramer

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

### WsServer

Combined WebSocket acceptor and framer.

Prefixes:

* `ws-upgrade:`
* `upgrade-ws:`
* `ws-u:`
* `u-ws:`

### WsUpgrade

Send HTTP/1 WebSocket upgrade to specified stream-oriented connection and accept and parse a reply,
then connects (i.e. exchanges bytes) the downstream connection to upstream.

Does not provide WebSocket framing.

Prefixes:

* `ws-request:`
* `ws-r:`

# Scenario functions

Prior to doing any network things, Websocat prepares a Scenario (Websocat Rhai Script) based on you command line options.
Scenarios are less stable than usual Websocat API, but allow fine tuning Websocat behaviour.
You can view scenarios using `--dump-spec` option and execute them the with `-x` option.

The following functions and methods are used in scenarios:

## Child::kill

Terminate a child process.
`Child` instance cannot be used after this.

Returns `Hangup`

## Child::socket

Convert the child process handle to a Stream Socket of its stdin and stdout (but not stderr).
In case of non-piped (`2`) fds, the resulting socket would be incomplete.

Returns `StreamSocket`

## Child::take_stderr

Take stderr handle as a Stream Reader (i.e. half-socket).
In case of non-piped (`2`) fds, the handle would be null

Returns `StreamRead`

## Child::wait

Obtain a Hangup handle that resovles when child process terminates.
`Child` instance cannot be used after this.

Returns `Hangup`

## Command::arg

Add one command line argument to the array

Parameters:

* arg (`String`)

Returns `()`

## Command::arg0

Override process's name / zeroeth command line argument on Unix.

Parameters:

* arg0 (`String`)

Returns `()`

## Command::arg0_osstr

Override process's name / zeroeth command line argument on Unix.

Parameters:

* arg0 (`OsString`)

Returns `()`

## Command::arg_osstr

Add one possibly non-UTF8 command line argument to the array

Parameters:

* arg (`OsString`)

Returns `()`

## Command::chdir

Change current directory for the subprocess.

Parameters:

* dir (`String`)

Returns `()`

## Command::chdir_osstr

Change current directory for the subprocess.

Parameters:

* dir (`OsString`)

Returns `()`

## Command::configure_fds

Configure what to do with subprocess's stdin, stdout and stderr

Each numeric argument accepts the following values:
* `0` meaning the fd will be /dev/null-ed.
* `1` meaning leave it connected to Websocat's fds.
* `2` meaning we can capture process's input or output.

Parameters:

* stdin (`i64`)
* stdout (`i64`)
* stderr (`i64`)

Returns `()`

## Command::dup2

`dup2` specified file descriptor over specified file descriptor numbers in the executed process

Parameters:

* source_fd (`i64`)
* destination_fds (`rhai::Dynamic`)
* set_to_blocking (`bool`)

Returns `()`

## Command::env

Add or set environtment variable for the subprocess

Parameters:

* key (`String`)
* value (`String`)

Returns `()`

## Command::env_clear

Clear all environment variables for the subprocess.

Returns `()`

## Command::env_osstr

Add or set environtment variable for the subprocess (possibly non-UTF8)

Parameters:

* key (`OsString`)
* value (`OsString`)

Returns `()`

## Command::env_remove

Add or set environtment variable for the subprocess.

Parameters:

* key (`String`)

Returns `()`

## Command::env_remove_osstr

Add or set environtment variable for the subprocess.

Parameters:

* key (`OsString`)

Returns `()`

## Command::execute

Spawn the prepared subprocess. What happens next depends on used `Child::` methods.

Returns `Child`

## Command::execute_for_output

Execute the prepared subprocess and wait for its exit, storing
output of stdout and stderr in memory.
Status code the callback receives follows similar rules as in `subprocess_execute_for_status`.
Second and third arguments of the callback are stdout and stderr respectively.

Parameters:

* continuation (`Fn(i64, Vec<u8>, Vec<u8>) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

## Command::execute_for_status

Execute the prepared subprocess and wait for its exit code
Callback receives exit code or `-1` meaning that starting failed
or `-2` meaning the process exited because of signal

Parameters:

* continuation (`Fn(i64) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

## Command::execve

Substitude Websocat process with the prepared command, abandoning other connections if they exist.

Returns `Child`

## Command::gid

Set subprocess's uid on Unix.

Parameters:

* gid (`i64`)

Returns `()`

## Command::raw_windows_arg

Add literal, unescaped text to Windows's command line

Parameters:

* arg (`OsString`)

Returns `()`

## Command::uid

Set subprocess's uid on Unix.

Parameters:

* uid (`i64`)

Returns `()`

## Command::windows_creation_flags

Set Windows's process creation flags.

Parameters:

* flags (`i64`)

Returns `()`

## DatagramSocketSlot::send

Put DatagramSocket into its slot, e.g. to initialize a reuser.

Acts immediately and returns a dummy task just as a convenience (to make Rhai scripts typecheck).

Parameters:

* socket (`DatagramSocket`)

Returns `Task`

## SimpleReuser::connect

Obtain a shared DatagramSocket pointing to the socket that was specified as `inner` into `simple_reuser` function.

Returns `DatagramSocket`

## SimpleReuserListener::maybe_init_then_connect

Initialize a persistent, shared DatagramSocket connection available for multiple clients (or just obtain a handle to it)

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* initializer (`Fn(DatagramSocketSlot) -> Task`) - Callback that is called on first call of this function and skipped on the rest (unless `recover` is set and needed) The callback is supposed to send a DatagramSocket to the slot.
* continuation (`Fn(DatagramSocket) -> Task`) - Callback that is called every time

Returns `Task`

Options:

* connect_again (`bool`) - Do not cache failed connection attempts, retry initialisation if a new client arrive. Note that successful, but closed connections are not considered failed and that regard and will stay cached. (use autoreconnect to handle that case)
* disconnect_on_broken_message (`bool`) - Drop underlying connection if some client leaves in the middle of writing a message, leaving us with unrecoverably broken message.

## TriggerableEvent::take_hangup

Take the waitable part (Hangup) from an object created by `triggerable_event_create`

Returns `Hangup`

## TriggerableEvent::take_trigger

Take the activatable part from an object created by `triggerable_event_create`

Returns `TriggerableEventTrigger`

## TriggerableEventTrigger::fire

Trigger the activatable part from an object created by `triggerable_event_create`.
This should cause a hangup even on the associated Hangup object.

Returns `()`

## async_fd

Use specified file descriptor for input/output, retuning a StreamSocket.

If you want it as DatagramSocket, just wrap it in a `chunks` wrapper.

May cause unsound behaviour if misused.

Parameters:

* fd (`i64`)

Returns `StreamSocket`

## b64str

Decode base64 string to another string

Parameters:

* x (`&str`)

Returns `String`

## connect_registry_stream

Connect to an intra-Websocat stream socket listening on specified virtual address.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* addr (`String`)
* max_buf_size (`usize`) - Maximum size of buffer for data in flight

## connect_seqpacket

Connect to a SOCK_SEQPACKET UNIX stream socket

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`)
* continuation (`Fn(DatagramSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* abstract (`bool`) - On Linux, connect ot an abstract-namespaced socket instead of file-based
* text (`bool`) - Mark received datagrams as text
* max_send_datagram_size (`usize`) - Default defragmenter buffer limit

## connect_tcp

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* addr (`SocketAddr`)

## connect_tcp_race

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* addrs (`Vec<SocketAddr>`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

## connect_unix

Connect to a UNIX stream socket of some kind

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* abstract (`bool`) - On Linux, connect to an abstract-namespaced socket instead of file-based

## copy_bytes

Forward unframed bytes from source to sink

Parameters:

* from (`StreamRead`) - stream source to read from
* to (`StreamWrite`) - stream sink to write to

Returns `Task` - task that finishes when forwarding finishes or exists with an error

## copy_packets

Copy packets from one datagram stream (half-socket) to a datagram sink.

Parameters:

* from (`DatagramRead`)
* to (`DatagramWrite`)

Returns `Task`

## create_stdio

Obtain a stream socket made of stdin and stdout.
This spawns a OS thread to handle interactions with the stdin/stdout and may be inefficient.

Returns `StreamSocket`

## datagram_logger

Wrap datagram socket in an overlay that logs every inner read and write to stderr.
Stderr is assumed to be always available. Backpressure would cause
whole process to stop serving connections and inability to log
may abort the process.

It is OK if a read or write handle of the source socket is null - resulting socket
would also be incomplete. This allows to access the logger having only reader
or writer instead of a complete socket.

This component is not performance-optimised and is intended for mostly for debugging.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`DatagramSocket`)

Returns `DatagramSocket`

Options:

* verbose (`bool`) - Show more messages and more info within messages
* read_prefix (`Option<String>`) - Prepend this instead of "READ " to each line printed to stderr
* write_prefix (`Option<String>`) - Prepend this instead of "WRITE " to each line printed to stderr
* omit_content (`bool`) - Do not log full content of the stream, just the chunk lengths.
* hex (`bool`) - Use hex lines instead of string literals with espaces
* include_timestamps (`bool`) - Also print relative timestamps for each log message

## display_pkts

Sample sink for packets for demostration purposes

Returns `DatagramWrite`

## drop

Attempt to drop a socket or task or other handle

Parameters:

* x (`Dynamic`)

Returns `()`

## dummy_datagram_socket

Create datagram socket with a source handle that continuously emits
EOF-marked empty buffers and a sink  handle that ignores all incoming data
and null hangup handle.

Can also be used a source of dummies for individual directions, with
`take_sink_part` and `take_source_part` functions

Returns `DatagramSocket`

## dummy_stream_socket

Create stream socket with a read handle that emits EOF immediately,
write handle that ignores all incoming data and null hangup handle.

Can also be used a source of dummies for individual directions, with
`take_read_part` and `take_write_part` functions

Returns `StreamSocket`

## dummy_task

A task that immediately finishes

Returns `Task`

## empty_hangup_handle

Create null Hangup handle

Returns `Hangup`

## exchange_bytes

Copy bytes between two stream-oriented sockets

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* s1 (`StreamSocket`)
* s2 (`StreamSocket`)

Returns `Task`

Options:

* unidirectional (`bool`) - Transfer data only from s1 to s2
* unidirectional_reverse (`bool`) - Transfer data only from s2 to s1
* exit_on_eof (`bool`) - abort one transfer direction when the other reached EOF
* unidirectional_late_drop (`bool`) - keep inactive transfer direction handles open
* buffer_size_forward (`Option<usize>`) - allocate this amount of buffers for transfer from s1 to s2
* buffer_size_reverse (`Option<usize>`) - allocate this amount of buffers for transfer from s2 to s1

## exchange_packets

Exchange packets between two datagram-oriented sockets

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* s1 (`DatagramSocket`)
* s2 (`DatagramSocket`)

Returns `Task`

Options:

* unidirectional (`bool`) - Transfer data only from s1 to s2
* unidirectional_reverse (`bool`) - Transfer data only from s2 to s1
* exit_on_eof (`bool`) - abort one transfer direction when the other reached EOF
* unidirectional_late_drop (`bool`) - keep inactive transfer direction handles open
* buffer_size_forward (`Option<usize>`) - allocate this amount of buffers for transfer from s1 to s2
* buffer_size_reverse (`Option<usize>`) - allocate this amount of buffers for transfer from s2 to s1

## exit_process

Exit Websocat process. If WebSocket is serving multiple connections, they all get aborted.

Parameters:

* code (`i64`)

Does not return anything.

## file_socket

Open specifid file and read/write it.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* write (`bool`) - Open specified file for writing, not reading
* append (`bool`) - Open specified file for appending, not reading
* no_overwrite (`bool`) - Do not overwrite existing files, instead use modified randomized name. Only relevant for `write` mode.
* auto_rename (`bool`) - Do not overwrite existing files, instead use modified randomized name. Only relevant for `write` mode.

## get_fd

Get underlying file descriptor from a socket, or -1 if is cannot be obtained

Parameters:

* x (`Dynamic`)

Returns `i64`

## get_ip

Extract IP address from SocketAddr

Parameters:

* sa (`&mut SocketAddr`)

Returns `String`

## get_port

Extract port from SocketAddr

Parameters:

* sa (`&mut SocketAddr`)

Returns `i64`

## handle_hangup

Spawn a task that calls `continuation` when specified socket hangup handle fires

Parameters:

* hangup (`Hangup`)
* continuation (`Fn() -> Task`) - Rhai function that will be called to continue processing

Returns `()`

## hangup2task

Convert a hangup token into a task. I don't know why this may be needed.

Parameters:

* hangup (`Hangup`)

Returns `Task`

## http1_client

Converts a downstream stream socket into a HTTP 1 client, suitable for sending e.g. WebSocket upgrade request.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)

Returns `Http1Client`

## http1_serve

Converts a downstream stream socket into a HTTP 1 server, suitable for accepting e.g. WebSocket upgrade request.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)
* continuation (`Fn(IncomingRequest, Hangup, i64) -> OutgoingResponse`) - Rhai function that will be called to continue processing

Returns `Task`

## length_prefixed_chunks

Convert downstream stream socket into upstream packet socket using a byte separator

If you want just source or sink conversion part, create incomplete socket, use this function, then extract the needed part from resulting incomplete socket.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* x (`StreamSocket`)

Returns `DatagramSocket`

Options:

* length_mask (`u64`) - Maximum message length that can be encoded in header, power of two minus one
* nbytes (`usize`) - Number of bytes in header field
* max_message_size (`usize`) - Maximum size of a message that can be encoded, unless `continuations` is set to true. Does not affect decoded messages.
* little_endian (`bool`) - Encode header as a little-endian number instead of big endian
* skip_read_direction (`bool`) - Inhibit adding header to data transferred in read direction, pass byte chunks unmodifed
* skip_write_direction (`bool`) - Inhibit adding header to data transferred in read direction, pass byte chunks unmodifed
* continuations (`Option<u64>`) - Do not defragment written messages,.write WebSocket frames instead of messages (and `or` specified number into the header).
* controls (`Option<u64>`) - Also write pings, pongs and CloseFrame messages, setting specified bit (pre-shifted) in header and prepending opcode in condent. Length would include this prepended byte.  Affects read direction as well, allowing manually triggering WebSocket control messages.
* tag_text (`Option<u64>`) - Set specified pre-shifted bit in header when dealing with text WebSocket messages. Note that with continuations, messages can be split into fragments in middle of a UTF-8 characters.

## line_chunks

Convert downstream stream socket into upstream packet socket using a byte separator

If you want just source or sink conversion part, create incomplete socket, use this function, then extract the needed part from resulting incomplete socket.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* x (`StreamSocket`)

Returns `DatagramSocket`

Options:

* separator (`Option<u8>`) - Use this byte as a separator. Defaults to 10 (\n).
* separator_n (`Option<usize>`) - Use this number of repetitions of the specified byte to consider it as a separator. Defaults to 1.
* substitute (`Option<u8>`) - When framing messages, look for byte sequences within the message that may alias with the separator and substitute last byte of such pseudo-separators with this byte value.  If active, leading and trailing separator bytes are also removed from the datagrams

## listen_registry_stream

Listen for intra-Websocat stream socket connections on a specified virtual address

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* addr (`String`)
* autospawn (`bool`) - Automatically spawn a task for each accepted connection
* oneshot (`bool`) - Exit listening loop after processing a single connection

## listen_seqpacket

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`) - Path to a socket file to create, name of abstract address to use or empty string if `fd` is used.
* when_listening (`Fn() -> Task`) - Called once after the port is bound
* on_accept (`Fn(DatagramSocket) -> Task`) - Call on each incoming connection

Returns `Task`

Options:

* fd (`Option<i32>`) - Inherited file descriptor to accept connections from
* named_fd (`Option<String>`) - Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
* fd_force (`bool`) - Skip socket type check when using `fd`.
* abstract (`bool`) - On Linux, connect ot an abstract-namespaced socket instead of file-based
* chmod (`Option<u32>`) - Change filesystem mode (permissions) of the file after listening
* autospawn (`bool`) - Automatically spawn a task for each accepted connection
* text (`bool`) - Mark received datagrams as text
* oneshot (`bool`) - Exit listening loop after processing a single connection
* max_send_datagram_size (`usize`) - Default defragmenter buffer limit

## listen_tcp

Listen TCP socket at specified address

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* when_listening (`Fn(SocketAddr) -> Task`) - Called once after the port is bound
* on_accept (`Fn(StreamSocket, SocketAddr) -> Task`) - Called on each connection

Returns `Task`

Options:

* addr (`Option<SocketAddr>`) - Socket address to bind listening socket tp
* fd (`Option<i32>`) - Inherited file descriptor to accept connections from
* named_fd (`Option<String>`) - Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
* fd_force (`bool`) - Skip socket type check when using `fd`.
* autospawn (`bool`) - Automatically spawn a task for each accepted connection
* oneshot (`bool`) - Exit listening loop after processing a single connection

## listen_unix

Listen UNIX or abstract socket

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`) - Path to a socket file to create, name of abstract address to use or empty string if `fd` is used.
* when_listening (`Fn() -> Task`) - Called once after the port is bound
* on_accept (`Fn(StreamSocket) -> Task`) - Called on each accepted connection

Returns `Task`

Options:

* fd (`Option<i32>`) - Inherited file descriptor to accept connections from
* named_fd (`Option<String>`) - Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
* fd_force (`bool`) - Skip socket type check when using `fd`.
* abstract (`bool`) - On Linux, listen an abstract-namespaced socket instead of file-based
* chmod (`Option<u32>`) - Change filesystem mode (permissions) of the file after listening
* autospawn (`bool`) - Automatically spawn a task for each accepted connection
* oneshot (`bool`) - Exit listening loop after processing a single connection

## literal_socket

Create a stream socket with a read handle emits specified data, then EOF; and
write handle that ignores all incoming data and null hangup handle.

Parameters:

* data (`String`)

Returns `StreamSocket`

## literal_socket_base64

Create a stream socket with a read handle emits specified data, then EOF; and
write handle that ignores all incoming data and null hangup handle.

Parameters:

* data (`String`)

Returns `StreamSocket`

## lookup_host

Perform a DNS lookup of the specified hostname and call a continuation with the list of IPv4 and IPv6 socket addresses

Parameters:

* addr (`String`)
* continuation (`Fn(Vec<SocketAddr>) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

## make_socket_addr

Build SocketAddr from IP and port

Parameters:

* ip (`&str`)
* port (`i64`)

Returns `SocketAddr`

## mock_stream_socket

Create special testing stream socket that emits user-specified data in user-specified chunks
and verifies that incoming data matches specified samples.

If something is unexpected, Websocat will exit (panic).

Argument is a specially formatted string describing operations, separated by `|` character.

Operations:

* `R` - make the socket return specified chunk of data
* `W` - make the socket wait for incoming data and check if it matches the sample
* 'ER' / `EW` - inject read or write error
* 'T0` ... `T9` - sleep for some time interval, from small to large.

Example: `R hello|R world|W ping |R pong|T5|R zero byte \0 other escapes \| \xff \r\n\t|EW`

Parameters:

* content (`String`)

Returns `StreamSocket`

## null_datagram_socket

Create datagram socket with null read, write and hangup handles.
Use `put_source_part` and `put_sink_part` to fill in the data transfer directions.

Returns `DatagramSocket`

## null_stream_socket

Create stream socket with null read, write and hangup handles.
Use `put_read_part` and `put_write_part` to fill in the data transfer directions.

Returns `StreamSocket`

## osstr_base64_unchecked_encoded_bytes

Decode base64 buffer and interpret using Rust's `OsString::from_encoded_bytes_unchecked`.
This format is not intended to be portable and is mostly for internal use within Websocat.

Parameters:

* x (`String`)

Returns `OsString`

## osstr_base64_unix_bytes

On Unix or WASI platforms, decode base64 buffer and convert it OsString.

Parameters:

* x (`String`)

Returns `OsString`

## osstr_base64_windows_utf16le

On Windows, decode base64 buffer and convert it OsString.

Parameters:

* x (`String`)

Returns `OsString`

## osstr_str

Convert a usual UTF-8 string to an OsString

Parameters:

* x (`String`)

Returns `OsString`

## parallel

Execute specified tasks in parallel, waiting them all to finish.

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## pre_triggered_hangup_handle

Create a Hangup handle that immediately resolves (i.e. signals hangup)

Returns `Hangup`

## print_stderr

Print a string to stderr (synchronously)

Parameters:

* x (`&str`)

Returns `()`

## print_stdout

Print a string to stdout (synchronously)

Parameters:

* x (`&str`)

Does not return anything.

## put_hangup_part

Modify Socket, filling in the hangup signal with the specified one

Parameters:

* h (`Dynamic`)
* x (`Hangup`)

Returns `()`

## put_read_part

Modify stream-oriented Socket, filling in the read direction with the specified one

Parameters:

* h (`StreamSocket`)
* x (`StreamRead`)

Returns `()`

## put_sink_part

Modify datagram-oriented Socket, filling in the write direction with the specified one

Parameters:

* h (`DatagramSocket`)
* x (`DatagramWrite`)

Returns `()`

## put_source_part

Modify datagram-oriented Socket, filling in the read direction with the specified one

Parameters:

* h (`DatagramSocket`)
* x (`DatagramRead`)

Returns `()`

## put_write_part

Modify stream-oriented Socket, filling in the write direction with the specified one

Parameters:

* h (`StreamSocket`)
* x (`StreamWrite`)

Returns `()`

## race

Execute specified tasks in parallel, aborting all others if one of them finishes.

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## read_chunk_limiter

Transform stream source so that reads become short reads if there is enough data. For development and testing.

Parameters:

* x (`StreamRead`)
* limit (`i64`)

Returns `StreamRead`

## read_stream_chunks

Convert a stream source to a datagram source

Parameters:

* x (`StreamRead`)

Returns `DatagramRead`

## sequential

Execute specified tasks in order, starting another and previous one finishes.

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## simple_reuser

Create object that multiplexes multiple DatagramSocket connections into one,
forwarding inner reads to arbitrary outer readers.

If inner socket disconnects, reuser will not attempt to reestablish the connection

Parameters:

* inner (`DatagramSocket`) - Datagram socket to multiplex connections to
* disconnect_on_torn_datagram (`bool`) - Drop inner connection when user begins writing a message, but leaves before finishing it, leaving inner connection with incomplete message that cannot ever be completed. If `false`, the reuser would commit the torn message and continue processing.

Returns `SimpleReuser`

## simple_reuser_listener

Create an inactive SimpleReuserListener.
It becomes active when `maybe_init_then_connect` is called the first time

Returns `SimpleReuserListener`

## sleep_ms

A task that finishes after specified number of milliseconds

Parameters:

* ms (`i64`)

Returns `Task`

## spawn_task

Start execution of the specified task in background

Parameters:

* task (`Task`)

Does not return anything.

## str

Turn any object into some string representation

Parameters:

* x (`Dynamic`)

Returns `String`

## stream_chunks

Convert a stream socket to a datagram socket. Like write_stream_chunks + read_stream_chunks while also preserving the hangup signal.

Parameters:

* x (`StreamSocket`)

Returns `DatagramSocket`

## stream_logger

Wrap stream socket in an overlay that logs every inner read and write to stderr.
Stderr is assumed to be always available. Backpressure would cause
whole process to stop serving connections and inability to log
may abort the process.

It is OK a if read or write handle of the source socket is null - resulting socket
would also be incomplete. This allows to access the logger having only reader
or writer instead of a complete socket.

This component is not performance-optimised and is intended for mostly for debugging.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)

Returns `StreamSocket`

Options:

* verbose (`bool`) - Show more messages and more info within messages
* read_prefix (`Option<String>`) - Prepend this instead of "READ " to each line printed to stderr
* write_prefix (`Option<String>`) - Prepend this instead of "WRITE " to each line printed to stderr
* omit_content (`bool`) - Do not log full content of the stream, just the chunk lengths.
* hex (`bool`) - Use hex lines instead of string literals with espaces
* include_timestamps (`bool`) - Also print relative timestamps for each log message

## subprocess_new

Prepare subprocess, setting up executable name. See `Command::` methods for further steps.

Parameters:

* program_name (`String`)

Returns `Command`

## subprocess_new_osstr

Prepare subprocess, setting up possibly non-UTF8 executable name.  See `Command::` methods for further steps.

Parameters:

* program_name (`OsString`)

Returns `Command`

## system

Simplified function to just execute a command line

Parameters:

* cmdline (`&str`)

Returns `Hangup`

## take_hangup_part

Modify Socket, taking the hangup signal part, if it is set.

Parameters:

* h (`Dynamic`)

Returns `Hangup`

## take_read_part

Modify stream-oriented Socket, taking the read part and returning it separately. Leaves behind an incomplete socket.

Parameters:

* h (`StreamSocket`)

Returns `StreamRead`

## take_sink_part

Modify datagram-oriented Socket, taking the write part and returning it separately. Leaves behind an incomplete socket.

Parameters:

* h (`DatagramSocket`)

Returns `DatagramWrite`

## take_source_part

Modify datagram-oriented Socket, taking the read part and returning it separately. Leaves behind an incomplete socket.

Parameters:

* h (`DatagramSocket`)

Returns `DatagramRead`

## take_write_part

Modify stream-oriented Socket, taking the write part and returning it separately. Leaves behind an incomplete socket.

Parameters:

* h (`StreamSocket`)

Returns `StreamWrite`

## task2hangup

Create hangup handle that gets triggered when specified task finishes.

Parameters:

* task (`Task`)
* mode (`i64`) - 0 means unconditionally, 1 means only when task has failed, 2 means only when task has succeeded.

Returns `Hangup`

## task_wrap

Create a Task that runs specified Rhai code when scheduled.

Parameters:

* continuation (`Fn() -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

## timeout_ms_hangup_handle

Create a Hangup handle that resolves after specific number of milliseconds

Parameters:

* ms (`i64`)

Returns `Hangup`

## tls_client

Perform TLS handshake using downstream stream-oriented socket, then expose stream-oriented socket interface to upstream that encrypts/decryptes the data.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* connector (`Arc<tokio_native_tls::TlsConnector>`)
* inner (`StreamSocket`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* domain (`String`)

## tls_client_connector

Create environment for using TLS clients.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function

Returns `Arc<tokio_native_tls::TlsConnector>`

Options:

* min_protocol_version (`Option<String>`)
* max_protocol_version (`Option<String>`)
* root_certificates_pem (`Vec<String>`)
* root_certificates_der_base64 (`Vec<String>`)
* disable_built_in_roots (`bool`)
* request_alpns (`Vec<String>`)
* danger_accept_invalid_certs (`bool`)
* danger_accept_invalid_hostnames (`bool`)
* no_sni (`bool`)

## triggerable_event_create

Create new one-time synchromisation object that allows to trigger a hangup event explicitly from Rhai code.

Returns `TriggerableEvent`

## trivial_pkts

Sample source of packets for demostration purposes

Returns `DatagramRead`

## udp_server

Create a single Datagram Socket that is bound to a UDP port,
typically for connecting to a specific UDP endpoint

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* when_listening (`Fn(SocketAddr) -> Task`) - Called once after the port is bound
* on_accept (`Fn(DatagramSocket, SocketAddr) -> Task`) - Called when new client is sending us datagrams

Returns `Task`

Options:

* bind (`Option<SocketAddr>`) - Specify address to bind the socket to.
* fd (`Option<i32>`) - Inherited file descriptor to accept connections from
* named_fd (`Option<String>`) - Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
* fd_force (`bool`) - Skip socket type check when using `fd`.
* timeout_ms (`Option<u64>`) - Mark the conection as closed when this number of milliseconds elapse without a new datagram from associated peer address
* max_clients (`Option<usize>`) - Maximum number of simultaneously connected clients. If exceed, stale clients (based on the last received datagram) will be hung up.
* buffer_size (`Option<usize>`) - Buffer size for receiving UDP datagrams. Default is 4096 bytes.
* qlen (`Option<usize>`) - Queue length for distributing received UDP datagrams among spawned DatagramSocekts Defaults to 1.
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compiant text WebSocket messages.
* backpressure (`bool`) - In case of one slow client handler, delay incoming UDP datagrams instead of dropping them
* inhibit_send_errors (`bool`) - Do not exit if `sendto` returned an error.
* max_send_datagram_size (`usize`) - Default defragmenter buffer limit

## udp_socket

Create a single Datagram Socket that is bound to a UDP port,
typically for connecting to a specific UDP endpoint

The node does not have it's own buffer size - the buffer is supplied externally

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function

Returns `DatagramSocket`

Options:

* addr (`SocketAddr`) - Send datagrams to and expect datagrams from this address. Specify neutral address like 0.0.0.0:0 to blackhole outgoing packets until correct address is determined.
* fd (`Option<i32>`) - Inherited file descriptor to accept connections from
* named_fd (`Option<String>`) - Inherited file named (`LISTEN_FDNAMES``) descriptor to accept connections from
* fd_force (`bool`) - Skip socket type check when using `fd`.
* bind (`Option<SocketAddr>`) - Specify address to bind the socket to. By default it binds to `0.0.0.0:0` or `[::]:0`
* sendto_mode (`bool`) - Use `sendto` instead of `connect` + `send`. This mode ignores ICMP reports that target is not reachable.
* allow_other_addresses (`bool`) - Do not filter out incoming datagrams from addresses other than `addr`. Useless without `sendto_mode`.
* redirect_to_last_seen_address (`bool`) - Send datagrams to address of the last seen incoming datagrams, using `addr` only as initial address until more data is received. Useless without `allow_other_addresses`. May have security implications.
* connect_to_first_seen_address (`bool`) - When using `redirect_to_last_seen_address`, lock the socket to that address, preventing more changes and providing disconnects. Useless without `redirect_to_last_seen_address`.
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compiant text WebSocket messages.
* inhibit_send_errors (`bool`) - Do not exit if `sendto` returned an error.
* max_send_datagram_size (`usize`) - Default defragmenter buffer limit

## unlink_file

Parameters:

* path (`OsString`)
* bail_if_fails (`bool`) - Emit error if unlinking fails.

Returns `()`

## write_buffer

Wrap stream writer in a buffering overlay that may accumulate data,
e.g. to write in one go on flush

Parameters:

* inner (`StreamWrite`)
* capacity (`i64`)

Returns `StreamWrite`

## write_chunk_limiter

Transform stream sink so that writes become short writes if the buffer is too large. For development and testing.

Parameters:

* x (`StreamWrite`)
* limit (`i64`)

Returns `StreamWrite`

## write_stream_chunks

Convert a stream sink to a datagram sink

Parameters:

* x (`StreamWrite`)

Returns `DatagramWrite`

## ws_accept

Perform WebSocket server handshake, then recover the downstream stream socket that was used for `http_server`.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* custom_headers (`rhai::Map`)
* rq (`IncomingRequest`)
* close_handle (`Hangup`)
* fd (`i64`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `OutgoingResponse`

Options:

* lax (`bool`) - Do not check incoming headers for correctness
* omit_headers (`bool`) - Do not include any headers in response
* choose_protocol (`Option<String>`) - If client supplies Sec-WebSocket-Protocol and it contains this one, include the header in response.
* require_protocol (`bool`) - Fail incoming connection if Sec-WebSocket-Protocol lacks the value specified in `choose_protocol` field (or any protocol if `protocol_choose_first` is active).
* protocol_choose_first (`bool`) - Round trip Sec-WebSocket-Protocol from request to response, choosing the first protocol if there are multiple

## ws_decoder

Wrap downstream stream-orinted reader to make expose packet-orinted source using WebSocket framing

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamRead`)

Returns `DatagramRead`

Options:

* require_masked (`bool`) - Require decoded frames to be masked (i.e. coming from a client)
* require_unmasked (`bool`) - Require decoded frames to be masked (i.e. coming from a server)

## ws_encoder

Wrap downstream stream-orinted writer to make expose packet-orinted sink using WebSocket framing

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamWrite`)

Returns `DatagramWrite`

Options:

* masked (`bool`) - Use masking (i.e. client-style)
* no_flush_after_each_message (`bool`)
* no_close_frame (`bool`) - Do not emit ConnectionClose frame when shutting down
* shutdown_socket_on_eof (`bool`) - Shutdown downstream socket for writing when shutting down

## ws_upgrade

Perform WebSocket client handshake, then recover the downstream stream socket that was used for `http_client`.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* custom_headers (`rhai::Map`) - Additional request headers to include
* client (`Http1Client`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* url (`String`)
* host (`Option<String>`)
* lax (`bool`) - Do not check response headers for correctness. Note that some `Upgrade:` header is required to continue connecting.
* omit_headers (`bool`) - Do not include any headers besides 'Host' (if any) in request

## ws_wrap

Like ws_encoder + ws_decoder, but also set up automatic replier to WebSocket pings.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)

Returns `DatagramSocket`

Options:

* client (`bool`) - Mask outgoing frames and require unmasked incoming frames
* ignore_masks (`bool`) - Accept masked (unmasked) frames in client (server) mode.
* no_flush_after_each_message (`bool`) - Inhibit flushing of underlying stream writer after each compelte message
* no_close_frame (`bool`) - Do not emit ConnectionClose frame when writing part is getting shut down
* shutdown_socket_on_eof (`bool`) - Propagate upstream writer shutdown to downstream
* no_auto_buffer_wrap (`bool`) - Do not automatically wrap WebSocket frames writer in a write_buffer: overlay when it detects missing vectored writes support
* max_ping_replies (`Option<usize>`) - Stop replying to WebSocket pings after sending this number of Pong frames.


# Glossary

* Specifier - WebSocket URL, TCP socket address or other connection type Websocat recognizes, 
or an overlay that transforms other Specifier.
* Endpoint - leaf-level specifier that directly creates some sort of Socket, without requiring another Socket first.
* Overlay - intermediate specifier that transforms inner specifier. From overlay's viewpoint, inner socket is called Downstream and whatever uses the overlay is called Upstream.
* Socket - a pair of byte stream- or datagram-oriented data flows: write (sink) and read (source), optionally with a hangup signal. Can be stream- and packet-oriented.
* Incomplete socket - socket where one of direction (reader or writer) is absent (null). Not to be confused with half-shutdown socket that can be read, but not written.
* Scenario = Websocat Rhai Script - detailed instruction of how Websocat would perform its operation.
Normally it is generated automatically from CLI arguments, then executed; but you can separate 
those steps and customize the scenario to fine tune of how Websocat operates. Just like usual CLI API, 
Rhai functions API is also intended to be semver-stable API of Websocat.
* Scenario function - Rhai native function that Websocat registers with Rhai engine that can be used 
in Scenarios.
* Scenario Planner - part of Websocat implementation that parses command line arguments and prepares a Scenario
* Scenario Executor - part of Websocat implementation that executes a Scenario.
* CLI arguments - combination of a positional arguments (typically Specifiers) and various 
flags (e.g. `--binary`) and options (e.g. `--buffer-size 4096`) that affect Scenario Planner.
* Packet = Datagram = Message - A byte buffer with associated flags. Correspond to one WebSocket message. Within WebSocket, packets can be split to chunks, but that should not affect user-visible properties.
* Chunk = Frame - portion of data read or written to/from stream or datagram socket in one go. Maybe a fragment of a Message or be the whole Message.
* Task - a logical thread of execution. Rhai code is expected to create and combine some tasks. Typically each connection runs in its own task. Corresponds to Tokio tasks.
* Hangup - similar to Task, but used in context of signaling various events, especially abrupt reset of sockets.
* Specifier stack - Invididual components of a Specifier - Endpoint and a vector of Overlays.
* Left side, first specifier - first positional argument you have specified at the left side of the Websocat CLI invocation (maybe after some transformation). Designed to handle both one-time use connectors and multi-use listeners.
* Right side, second specifier - second positional argument of the Websocat CLI invocation (may be auto-generated). Designed for single-use things to attach to connections obtained from the Left side.
* Listener - Type of Specifier that waits for incoming connections, swapning a task with a Socket for each incoming connection.


