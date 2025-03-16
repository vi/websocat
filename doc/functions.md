<!-- Note: this file is auto-generated -->
{{#include functions_header.md}}
## Child::kill

Terminate a child process.
`Child` instance cannot be used after this.

Returns `Hangup`

## Child::socket

Convert the child process handle to a Stream Socket of its stdin and stdout (but not stderr).
In case of non-piped (`2`) FDs, the resulting socket would be incomplete.

Returns `StreamSocket`

## Child::take_stderr

Take stderr handle as a Stream Reader (i.e. half-socket).
In case of non-piped (`2`) FDs, the handle would be null

Returns `StreamRead`

## Child::wait

Obtain a Hangup handle that resolves when child process terminates.
`Child` instance cannot be used after this.

Returns `Hangup`

## Command::arg

Add one command line argument to the array

Parameters:

* arg (`String`)

Returns `()`

## Command::arg0

Override process's name / zeroth command line argument on Unix.

Parameters:

* arg0 (`String`)

Returns `()`

## Command::arg0_osstr

Override process's name / zeroth command line argument on Unix.

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

Add or set environment variable for the subprocess

Parameters:

* key (`String`)
* value (`String`)

Returns `()`

## Command::env_clear

Clear all environment variables for the subprocess.

Returns `()`

## Command::env_osstr

Add or set environment variable for the subprocess (possibly non-UTF8)

Parameters:

* key (`OsString`)
* value (`OsString`)

Returns `()`

## Command::env_remove

Add or set environment variable for the subprocess.

Parameters:

* key (`String`)

Returns `()`

## Command::env_remove_osstr

Add or set environment variable for the subprocess.

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

Substitute Websocat process with the prepared command, abandoning other connections if they exist.

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

Use specified file descriptor for input/output, returning a StreamSocket.

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

* abstract (`bool`) - On Linux, connect to an abstract-namespaced socket instead of file-based
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

Sample sink for packets for demonstration purposes

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

Open specified file and read/write it.

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
* skip_read_direction (`bool`) - Inhibit adding header to data transferred in read direction, pass byte chunks unmodified
* skip_write_direction (`bool`) - Inhibit adding header to data transferred in read direction, pass byte chunks unmodified
* continuations (`Option<u64>`) - Do not defragment written messages, write WebSocket frames instead of messages (and bitwise-or specified number into the header).
* controls (`Option<u64>`) - Also write pings, pongs and CloseFrame messages, setting specified bit (pre-shifted) in header and prepending opcode in content. Length would include this prepended byte.  Affects read direction as well, allowing manually triggering WebSocket control messages.
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
* abstract (`bool`) - On Linux, connect to an abstract-namespaced socket instead of file-based
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

* addr (`Option<SocketAddr>`) - Socket address to bind listening socket to
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
* `ER` / `EW` - inject read or write error
* `T0` ... `T9` - sleep for some time interval, from small to large.

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

## random_socket

Create a StreamSocket that reads random bytes (affected by --random-seed) and ignores writes

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function

Returns `StreamSocket`

Options:

* fast (`bool`) - Use small, less secure RNG instead of slower secure one.

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

## stdio_socket

Obtain a stream socket made of stdin and stdout.
This spawns a OS thread to handle interactions with the stdin/stdout and may be inefficient.

Returns `StreamSocket`

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

Perform TLS handshake using downstream stream-oriented socket, then expose stream-oriented socket interface to upstream that encrypts/decrypts the data.

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

Create new one-time synchronisation object that allows to trigger a hangup event explicitly from Rhai code.

Returns `TriggerableEvent`

## trivial_pkts

Sample source of packets for demonstration purposes

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
* timeout_ms (`Option<u64>`) - Mark the connection as closed when this number of milliseconds elapse without a new datagram from associated peer address
* max_clients (`Option<usize>`) - Maximum number of simultaneously connected clients. If exceed, stale clients (based on the last received datagram) will be hung up.
* buffer_size (`Option<usize>`) - Buffer size for receiving UDP datagrams. Default is 4096 bytes.
* qlen (`Option<usize>`) - Queue length for distributing received UDP datagrams among spawned DatagramSocekts Defaults to 1.
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compliant text WebSocket messages.
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
* tag_as_text (`bool`) - Tag incoming UDP datagrams to be sent as text WebSocket messages instead of binary. Note that Websocat does not check for UTF-8 correctness and may send non-compliant text WebSocket messages.
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

Wrap downstream stream-oriented reader to make expose packet-oriented source using WebSocket framing

Parameters:

* opts (`Dynamic`) - object map containing dynamic options to the function
* inner (`StreamRead`)

Returns `DatagramRead`

Options:

* require_masked (`bool`) - Require decoded frames to be masked (i.e. coming from a client)
* require_unmasked (`bool`) - Require decoded frames to be unmasked (i.e. coming from a server)

## ws_encoder

Wrap downstream stream-oriented writer to make expose packet-oriented sink using WebSocket framing

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
* no_flush_after_each_message (`bool`) - Inhibit flushing of underlying stream writer after each complete message
* no_close_frame (`bool`) - Do not emit ConnectionClose frame when writing part is getting shut down
* shutdown_socket_on_eof (`bool`) - Propagate upstream writer shutdown to downstream
* no_auto_buffer_wrap (`bool`) - Do not automatically wrap WebSocket frames writer in a write_buffer: overlay when it detects missing vectored writes support
* max_ping_replies (`Option<usize>`) - Stop replying to WebSocket pings after sending this number of Pong frames.

## zero_socket

Create a StreamSocket that reads zero bytes and ignores writes

Returns `StreamSocket`

