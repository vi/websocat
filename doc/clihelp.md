<!-- Note: this file is auto-generated -->
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
          execute specified file as Rhai script (e.g. resulting from --dump-spec option output)

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
          listen for WebSocket connections instead of establishing client WebSocket connection

      --log-verbose
          log more data from `log:` overlay

      --log-omit-content
          do not log full content of the data from `log:` overlay, just chunk lengths

      --log-hex
          use hex lines instead of escaped characters for `log:`` overlay

      --log-timestamps
          Include relative timestamps in log messages

      --log-traffic
          automatically insert `log:` overlay in an appropriate place to debug issues by displaying traffic chunks

      --ws-c-uri <WS_C_URI>
          URI for `ws-c:` overlay

      --read-buffer-limit <READ_BUFFER_LIMIT>
          parameter for read_chunk_limiter: overlay, defaults to 1

      --write-buffer-limit <WRITE_BUFFER_LIMIT>
          parameter for write_chunk_limiter: overlay, defaults to 1

      --separator <SEPARATOR>
          override byte value that separates stdin-supplied text WebSocket messages from each other from default '\n'

      --separator-n <SEPARATOR_N>
          require this number of newline (or other) bytes to separate WebSocket messages

      --separator-inhibit-substitution
          prevent mangling incoming text WebSocket by replacing `\n`  (or other separator sequence) with spaces (and trimming leading and trailing separator bytes)

      --separator-inline
          make separator (such as trailing \n) a part text WebSocket messages, do not remove it when splitting messages

  -0, --null-terminated
          Same as setting `--separator` to `0`. Make text mode messages separated by zero byte instead of newline

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
          On Unix, use dup2 and forward sockets directly to child processes (ignoring any overlays) instead of piping though stdin/stdout. Argument is comma-separated list of file descriptor slots to duplicate the socket into, e.g. `0,1` for stdin and stdout.
          
          This is a low-level option that is less tested than other things. Expect non-user-friendly error messages if misused.

      --exec-dup2-keep-nonblocking
          When using --exec-dup2, do not set inherited file descriptors to blocking mode

      --exec-dup2-execve
          on Unix, When using `--exec-dup2`, do not return to Websocat, instead substitute Websocat process with the given command

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
          Override buffer size for main data transfer session. Note that some overlays and endpoints may have separate buffers with separately adjustable sizes.
          
          Message can span multiple over multiple fragments and exceed this buffer size

  -n, --no-close
          Do not send WebSocket close message when there is no more data to send there

      --ws-no-flush
          Do not flush after each WebSocket frame

      --ws-shutdown-socket-on-eof
          Shutdown write direction of the underlying socket backing a WebSocket on EOF

      --ws-ignore-invalid-masks
          Do not fail WebSocket connections if masked frame arrives instead of unmasked or vice versa

      --ws-dont-check-headers
          Ignore absence or invalid values of `Sec-Websocket-*` things and just continue connecting

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

      --less-fixups
          Inhibit some optional transformations of specifier stacks, such as auto-inserting of a reuser

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

      --mirror-bufsize <MIRROR_BUFSIZE>
          Use specified buffer size for mirror: endpoint
          
          [default: 1024]

      --lengthprefixed-little-endian
          Use little-endian framing headers instead of big-endian for `lengthprefixed:` overlay

      --lengthprefixed-skip-read-direction
          Only affect one direction of the `lengthprefixed:` overlay, bypass transformation for the other one

      --lengthprefixed-skip-write-direction
          Only affect one direction of the `lengthprefixed:` overlay, bypass transformation for the other one

      --lengthprefixed-nbytes <LENGTHPREFIXED_NBYTES>
          Use this number of length header bytes for `lengthprefixed:` overlay
          
          [default: 4]

      --lengthprefixed-continuations
          Do not reassemble messages from fragments, stream them as chunks. Highest bit of the prefix would be set if the message is non-final

      --lengthprefixed-max-message-size <LENGTHPREFIXED_MAX_MESSAGE_SIZE>
          Maximum size of `lengthprefixed:` message (that needs to be reassembled from fragments)
          
          Connections would fail when messages exceed this size.
          
          Ignored if `--lengthprefixed-continuations` is active, but `nbytes`-based limitation can still fail connections.
          
          [default: 1048576]

      --lengthprefixed-include-control
          Include inline control messages (i.e. WebSocket pings or close frames) as content in `lengthprefixed:`.
          
          A bit would be set to signify a control message and opcode will be prepended as the first byte.
          
          When both continuations and controls are enabled, control messages may appear between continued data message chunks. Control messages can themselves be subject to continuations.

      --lengthprefixed-tag-text
          Set a bit in the prefix of `lengthprefixed:` frames when the frame denotes a text WebSocket message instead of binary

      --inhibit-pongs <INHIBIT_PONGS>
          Stop automatic replying to WebSocket pings after sending specified number of pongs. May be zero to just disable replying to pongs

      --global-timeout-ms <GLOBAL_TIMEOUT_MS>
          Abort whatever Websocat is doing after specified number of milliseconds, regardless of whether something is connected or not

      --global-timeout-force-exit
          Force process exit when global timeout is reached

      --sleep-ms-before-start <SLEEP_MS_BEFORE_START>
          Wait for this number of milliseconds before starting endpoints. Mostly intended for testing Websocat, in combination with --compose mode

      --stdout-announce-listening-ports
          Print a line to stdout when a port you requested to be listened is ready to accept connections

      --exec-after-listen <EXEC_AFTER_LISTEN>
          Execute this command line after binding listening port
          
          Connections are not actually accepted until the command exits (exit code is ignored)

      --exec-after-listen-append-port
          Append TCP or UDP port number to command line specified in --exec-after-listen
          
          Makes listening port "0" practical.

      --accept-from-fd
          Show dedicated error message explaining how to migrate Websocat1's --accept-from-fd to the new scheme

      --reuser-tolerate-torn-msgs
          When using `reuse-raw:` (including automatically inserted), do not abort connections on unrecoverable broken messages, instead produce a trimmed message and continue

      --compose
          Interpret special command line arguments like `&`, `;`, '^', `(` and `)` as separators for composed scenarios mode. This argument must come first.
          
          This allows to execute multiple sub-scenarios in one Websocat invocation, sequentially (;), in parallel (&) or in parallel with early exit (^). You can also use parentheses to combine disparate operations.

      --write-file-no-overwrite
          For `writefile:` endpoint, do not overwrite existing files

      --write-file-auto-rename
          For `writefile:` endpoint, do not overwrite existing files, instead use other, neighbouring file names

      --origin <ORIGIN>
          Add Origin HTTP header to Websocket client request

      --ua <UA>
          Add User-Agent HTTP header to Websocket client request

      --random-fast
          For `random:` endpoint, use smaller and faster RNG instead of secure one

      --write-splitoff <WRITE_SPLITOFF>
          Specify the write counterpart for `write-splitoff:` overlay. Expects a specifier like `tcp:127.0.0.1:1234`, like a positional argument

      --write-splitoff-omit-shutdown
          Do not write-shutdown the read part of a `write-splitoff:` overlay

      --filter <FILTER>
          Pass traffic through this socket prior to transfer data from left to right specifiers.
          
          The filter itself can be any specifier (including with overlays), e.g. `--filter=lines:tcp:127.0.0.1:1234`

      --filter-reverse <FILTER_REVERSE>
          Pass traffic through this socket prior to transfer data from right to left specifiers

      --async-fd-force
          Force using a file descriptor for `async-fd:` even when it cannot be registered for events.
          
          In case of EWOULDBLOCK Websocat would wait for some short time in a loop.
          
          In some cases the whole Websocat process may be blocked.

      --defragment-max-size <DEFRAGMENT_MAX_SIZE>
          Maximum buffered message size for `defragment:` overlay
          
          [default: 1048576]

      --tee <TEE>
          Copy output datagrams also to this specifier; also merge in incoming datagrams from this specifier
          
          May insert a `tee:` overlay automatically if not specified.

      --tee-propagate-failures
          Cause `tee:` overlay to fail datagram read or write if any (instead of all) nodes failed the operation

      --tee-propagate-eof
          Cause `tee:` overlay's reading direction to propagate EOF when any of the nodes signaled EOF instead of all of them

      --tee-tolerate-torn-msgs
          When using `tee:`, do not abort reading side of the connections on unrecoverable broken messages, instead produce a trimmed message and continue

      --tee-use-hangups
          Terminate `tee:` specifier when any of the tee nodes signal hangup

      --tee-use-first-hangup
          Terminate `tee:` specifier when main tee node (the one after `tee:` instead of `--tee`) signals hangup

      --enable-sslkeylog
          When built with rustls, write encryption infromation to a files based on `SSLKEYLOGFILE`  envionrment variable to assist decrypting traffic for analysis

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
  mirror:
  mock_stream_socket:
  random:
  readfile:
  registry-datagram-connect:
  registry-datagram-listen:
  registry-send:
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
  zero:

Short list of overlay prefixes:
  defragment:
  lengthprefixed:
  lines:
  log:
  read_chunk_limiter:
  reuse-raw:
  chunks:
  tee:
  tls:
  write_buffer:
  write_chunk_limiter:
  write-splitoff:
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

Use https://websocat.net/websocat4/ (or 'doc' directory in the source code) for reference of all Websocat functions
```
