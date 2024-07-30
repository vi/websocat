# Command-line interface

This section describes options, flags and specifiers of Websocat CLI.


## Endpoints

### Stdio

Console, terminal: read bytes from stdin, write bytes to stdout.

This endpoint cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

### TcpConnectByEarlyHostname



### TcpConnectByIp

(undocumented)

Prefixes:

* `tcp:`
* `tcp-connect:`
* `connect-tcp:`
* `tcp-c:`
* `c-tcp:`

### TcpConnectByLateHostname



### TcpListen

(undocumented)

Prefixes:

* `tcp-listen:`
* `listen-tcp:`
* `tcp-l:`
* `l-tcp:`

### UdpBind

Bind UDP socket to this address.
Commmand line options greatly affect this endpoint. It can be turned into a flexible UdpConnect analogue.

Prefixes:

* `udp-bind:`
* `bind-udp:`
* `udp-listen:`
* `listen-udp:`
* `udp-l:`
* `l-udp:`

### UdpConnect

Connect to this UDP socket. Note affected by `--udp-bind-*`` CLI options.

Prefixes:

* `udp:`
* `udp-connect:`
* `connect-udp:`
* `udp-c:`
* `c-udp:`

### UdpServer

Bind UDP socket and spawn a separate task for each client

Prefixes:

* `udp-server:`

### WsListen

(undocumented)

Prefixes:

* `ws-l:`

### WsUrl

(undocumented)

Prefixes:

* `ws://`

### WssUrl

(undocumented)

Prefixes:

* `wss://`


## Overlays

### LineChunks

(undocumented)

This overlay cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

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

### StreamChunks

(undocumented)

This overlay cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

### TlsClient

(undocumented)

Prefixes:

* `tls:`

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

(undocumented)

Prefixes:

* `ws-accept:`

### WsClient

Combined WebSocket upgrader and framer, but without TCP or TLS things
URI is taked from --ws-c-uri CLI argument
If it is not specified, it defaults to `/`, with a missing `host:` header

Prefixes:

* `ws-c:`

### WsFramer

(undocumented)

Prefixes:

* `ws-ll-client:`
* `ws-ll-server:`

### WsUpgrade

(undocumented)

This overlay cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

# Scenario functions

Those functions are used in Websocat Rhai Scripts (Scenarios):

## b64str

Decode base64 string to another string

Parameters:

* x (`&str`)

Returns `String`

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

## copy_bytes

Forward unframed bytes from source to sink

Parameters:

* from (`StreamRead`) - stream source to read from
* to (`StreamWrite`) - stream sink to write to

Returns `Task` - task that finishes when forwarding finishes or exists with an error

## copy_packets

Parameters:

* from (`DatagramRead`)
* to (`DatagramWrite`)

Returns `Task`

## create_stdio

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

## display_pkts

Returns `DatagramWrite`

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

Returns `Task`

## empty_hangup_handle

Returns `Hangup`

## exchange_bytes

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* s1 (`StreamSocket`)
* s2 (`StreamSocket`)

Returns `Task`

## exchange_packets

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* s1 (`DatagramSocket`)
* s2 (`DatagramSocket`)

Returns `Task`

## http1_client

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)

Returns `Http1Client`

## http1_serve

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)
* continuation (`Fn(IncomingRequest, Hangup) -> OutgoingResponse`) - Rhai function that will be called to continue processing

Returns `Task`

## line_chunks

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* x (`StreamSocket`)

Returns `DatagramSocket`

Options:

* separator (`Option<u8>`) - Use this byte as a separator. Defaults to 10 (\n).
* separator_n (`Option<usize>`) - Use this number of repetitions of the specified byte to consider it as a separator. Defaults to 1.
* substitute (`Option<u8>`) - When framing messages, look for byte sequences within the message that may alias with the separator and substitute last byte of such pseudo-separators with this byte value.  If active, leading and trailing separator bytes are also removed from the datagrams

## listen_tcp

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket, SocketAddr) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* addr (`SocketAddr`)
* autospawn (`bool`)

## lookup_host

Parameters:

* addr (`String`)
* continuation (`Fn(Vec<SocketAddr>) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

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

## parallel

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## put_hangup_part

Parameters:

* h (`Dynamic`)
* x (`Hangup`)

Returns `()`

## put_read_part

Parameters:

* h (`StreamSocket`)
* x (`StreamRead`)

Returns `()`

## put_sink_part

Parameters:

* h (`DatagramSocket`)
* x (`DatagramWrite`)

Returns `()`

## put_source_part

Parameters:

* h (`DatagramSocket`)
* x (`DatagramRead`)

Returns `()`

## put_write_part

Parameters:

* h (`StreamSocket`)
* x (`StreamWrite`)

Returns `()`

## read_chunk_limiter

Parameters:

* x (`StreamRead`)
* limit (`i64`)

Returns `StreamRead`

## read_stream_chunks

Parameters:

* x (`StreamRead`)

Returns `DatagramRead`

## sequential

Parameters:

* tasks (`Vec<Dynamic>`)

Returns `Task`

## sleep_ms

Parameters:

* ms (`i64`)

Returns `Task`

## spawn_task

Parameters:

* task (`Task`)

Does not return anything.

## stream_chunks

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

## subprocess

Start child process and interpret its stdin/stdout as a StreamSocket.

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* program (`String`)
* argv (`Vec<String>`)
* base64_args (`bool`) - Interpret `argv` as base64-encoded buffers instead of direct strings.

## subprocess_new

Prepare subprocess, setting up executable name.

Parameters:

* program_name (`String`)

Returns `Command`

## subprocess_new_osstr

Prepare subprocess, setting up possibly non-UTF8 executable name 

Parameters:

* program_name (`OsString`)

Returns `Command`

## take_hangup_part

Parameters:

* h (`Dynamic`)

Returns `Hangup`

## take_read_part

Parameters:

* h (`StreamSocket`)

Returns `StreamRead`

## take_sink_part

Parameters:

* h (`DatagramSocket`)

Returns `DatagramWrite`

## take_source_part

Parameters:

* h (`DatagramSocket`)

Returns `DatagramRead`

## take_write_part

Parameters:

* h (`StreamSocket`)

Returns `StreamWrite`

## tls_client

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* connector (`Arc<tokio_native_tls::TlsConnector>`)
* inner (`StreamSocket`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* domain (`String`)

## tls_client_connector

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

## trivial_pkts

Returns `DatagramRead`

## udp_server

Create a single Datagram Socket that is bound to a UDP port,
typically for connecting to a specific UDP endpoint

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(DatagramSocket, SocketAddr) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* bind (`SocketAddr`) - Specify address to bind the socket to.
* timeout_ms (`Option<u64>`) - Mark the conection as closed when this number of milliseconds elapse without a new datagram from associated peer address
* max_clients (`Option<usize>`) - Maximum number of simultaneously connected clients. If exceed, stale clients (based on the last received datagram) will be hung up.
* buffer_size (`Option<usize>`) - Buffer size for receiving UDP datagrams. Default is 4096 bytes.
* qlen (`Option<usize>`) - Queue length for distributing received UDP datagrams among spawned DatagramSocekts Defaults to 1.
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compiant text WebSocket messages.
* backpressure (`bool`) - In case of one slow client handler, delay incoming UDP datagrams instead of dropping them
* inhibit_send_errors (`bool`) - Do not exit if `sendto` returned an error.

## udp_socket

Create a single Datagram Socket that is bound to a UDP port,
typically for connecting to a specific UDP endpoint

The node does not have it's own buffer size - the buffer is supplied externally

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function

Returns `DatagramSocket`

Options:

* addr (`SocketAddr`) - Send datagrams to and expect datagrams from this address.
* bind (`Option<SocketAddr>`) - Specify address to bind the socket to. By default it binds to `0.0.0.0:0` or `[::]:0`
* sendto_mode (`bool`) - Use `sendto` instead of `connect` + `send`. This mode ignores ICMP reports that target is not reachable.
* allow_other_addresses (`bool`) - Do not filter out incoming datagrams from addresses other than `addr`. Useless without `sendto_mode`.
* redirect_to_last_seen_address (`bool`) - Send datagrams to address of the last seen incoming datagrams, using `addr` only as initial address until more data is received. Useless without `allow_other_addresses`. May have security implications.
* connect_to_first_seen_address (`bool`) - When using `redirect_to_last_seen_address`, lock the socket to that address, preventing more changes and providing disconnects. Useless without `redirect_to_last_seen_address`.
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compiant text WebSocket messages.
* inhibit_send_errors (`bool`) - Do not exit if `sendto` returned an error.

## write_buffer

Wrap stream writer in a buffering overlay that may accumulate data,
e.g. to write in one go on flush

Parameters:

* inner (`StreamWrite`)
* capacity (`i64`)

Returns `StreamWrite`

## write_chunk_limiter

Parameters:

* x (`StreamWrite`)
* limit (`i64`)

Returns `StreamWrite`

## write_stream_chunks

Parameters:

* x (`StreamWrite`)

Returns `DatagramWrite`

## ws_accept

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* rq (`IncomingRequest`)
* close_handle (`Hangup`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `OutgoingResponse`

Options:

* lax (`bool`)

## ws_decoder

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamRead`)

Returns `DatagramRead`

Options:

* require_masked (`bool`)
* require_unmasked (`bool`)

## ws_encoder

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamWrite`)

Returns `DatagramWrite`

Options:

* masked (`bool`)
* no_flush_after_each_message (`bool`)
* no_close_frame (`bool`)
* shutdown_socket_on_eof (`bool`)

## ws_upgrade

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* client (`Http1Client`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* url (`String`)
* host (`Option<String>`)
* lax (`bool`)

## ws_wrap

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamSocket`)

Returns `DatagramSocket`

Options:

* client (`bool`)
* ignore_masks (`bool`)
* no_flush_after_each_message (`bool`)
* no_close_frame (`bool`)
* shutdown_socket_on_eof (`bool`)
* no_auto_buffer_wrap (`bool`) - Do not automatically wrap WebSocket frames writer in a write_buffer: overlay when it detects missing vectored writes support


# Glossary

* Specifier - WebSocket URL, TCP socket address or other connection type Websocat recognizes, 
or an overlay that transforms other Specifier.
* Endpoint - leaf-level specifier that directly creates some sort of Socket
* Overlay - intermediate specifier that transforms inner specifier
* Socket - a pair of byte stream- or datagram-oriented data flows: write 
(to socket) and read (from socket), optionally with a hangup signal
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


