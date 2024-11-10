# Command-line interface

This section describes options, flags and specifiers of Websocat CLI.


## Endpoints

### AbstractConnect

Connect to the specified abstract-namespaced UNIX socket (Linux)

Prefixes:

* `abstract:`
* `abstract-connect:`
* `connect-abstract:`
* `abstract-c:`
* `c-abstract:`

### AbstractListen

Listen UNIX socket on specified abstract path (Linux)

Prefixes:

* `abstract-listen:`
* `listen-abstract:`
* `abstract-l:`
* `l-abstract:`

### AbstractSeqpacketConnect

Connect to specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name

Prefixes:

* `seqpacket-abstract:`
* `seqpacket-abstract-connect:`
* `seqpacket-abstract-c:`
* `abstract-seqpacket:`
* `abstract-seqpacket-connect:`
* `abstract-seqpacket-c:`

### AbstractSeqpacketListen

Listen specified UNIX SOCK_SEQPACKET socket by abstract (Linux) name

Prefixes:

* `seqpacket-abstract-listen:`
* `seqpacket-abstract-l:`
* `abstract-seqpacket-listen:`
* `abstract-seqpacket-l:`

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

### SeqpacketConnect

Connect to specified UNIX SOCK_SEQPACKET socket by path

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket:`
* `seqpacket-connect:`
* `connect-seqpacket:`
* `seqpacket-c:`
* `c-seqpacket:`

### SeqpacketListen

Listen specified UNIX SOCK_SEQPACKET socket

Unlike Websocat1, @-prefixed addresses do not get converted to Linux abstract namespace

Prefixes:

* `seqpacket-listen:`
* `listen-seqpacket:`
* `seqpacket-l:`
* `l-seqpacket:`

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

### UdpServer

Bind UDP socket and spawn a separate task for each client.
Connections get closed when there are too many active clients or by a timeout.

Prefixes:

* `udp-server:`

### UnixConnect

Connect to the specified UNIX socket path

Prefixes:

* `unix:`
* `unix-connect:`
* `connect-unix:`
* `unix-c:`
* `c-unix:`

### UnixListen

Listen specified UNIX socket path

Prefixes:

* `unix-listen:`
* `listen-unix:`
* `unix-l:`
* `l-unix:`

### WsListen

Listen for incoming WebSocket connections at specified TCP socket address.

Prefixes:

* `ws-l:`

### WsUrl

Connect to specified WebSocket plain (insecure) URL

Prefixes:

* `ws://`

### WssUrl

Connect to specified WebSocket TLS URL

Prefixes:

* `wss://`


## Overlays

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

### StreamChunks

Converts downstream stream-oriented socket to packet-oriented socket by chunking the stream arbitrarily (i.e. as syscalls happend to deliver the data)

May be automatically inserted in binary (`-b`) mode.

Prefixes:

* `chunks:`

### TlsClient

Establishes client-side TLS connection using specified stream-oriended downstream connection

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

Prefixes:

* `ws-ll-client:`
* `ws-ll-server:`

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

This overlay cannot be directly specified as a prefix to a positional CLI argument, there may be some other way to access it.

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

## b64str

Decode base64 string to another string

Parameters:

* x (`&str`)

Returns `String`

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

## dbg

Debug print something to stderr

Parameters:

* x (`Dynamic`)

Does not return anything.

## display_pkts

Sample sink for packets for demostration purposes

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

## handle_hangup

Spawn a task that calls `continuation` when specified socket hangup handle fires

Parameters:

* hangup (`Hangup`)
* continuation (`Fn()`) - Rhai function that will be called to continue processing

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
* continuation (`Fn(IncomingRequest, Hangup) -> OutgoingResponse`) - Rhai function that will be called to continue processing

Returns `Task`

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

## listen_seqpacket

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`)
* continuation (`Fn(DatagramSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* abstract (`bool`) - On Linux, connect ot an abstract-namespaced socket instead of file-based
* chmod (`Option<u32>`) - Change filesystem mode (permissions) of the file after listening
* autospawn (`bool`) - Automatically spawn a task for each accepted connection
* text (`bool`) - Mark received datagrams as text

## listen_tcp

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* continuation (`Fn(StreamSocket, SocketAddr) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* addr (`SocketAddr`)
* autospawn (`bool`)

## listen_unix

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* path (`OsString`)
* continuation (`Fn(StreamSocket) -> Task`) - Rhai function that will be called to continue processing

Returns `Task`

Options:

* abstract (`bool`) - On Linux, connect ot an abstract-namespaced socket instead of file-based
* chmod (`Option<u32>`) - Change filesystem mode (permissions) of the file after listening
* autospawn (`bool`) - Automatically spawn a task for each accepted connection

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

## trivial_pkts

Sample source of packets for demostration purposes

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


