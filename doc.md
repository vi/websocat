
# Websocat Reference (in progress)

Websocat has many command-line options and special format for positional arguments.

There are three main modes of websocat invocation:

* Simple client mode: `websocat wss://your.server/url`
* Simple server mode: `websocat -s 127.0.0.1:8080`
* Advanced [socat][1]-like mode: `websocat -t ws-l:127.0.0.1:8080 mirror:`

Ultimately in any of those modes websocat creates two connections and exchanges data between them.
If one of the connections is bytestream-oriented (for example the terminal stdin/stdout or a TCP connection), but the other is message-oriented (for example, a WebSocket or UDP) then websocat operates in lines: each line correspond to a message. Details of this are configurable by various options.

`ws-l:` or `mirror:` above are examples of address types. With the exception of special cases like WebSocket URL `ws://1.2.3.4/` or stdio `-`, websocat's positional argument is defined by this rule:

```
<specifier> ::= ( <overlay> ":" )* <addrtype> ":" [address]
```

Some address types may be "aliases" to other address types or combinations of overlays and address types.

[1]:http://www.dest-unreach.org/socat/doc/socat.html

# `--help=long`

"Advanced" options and flags are denoted by `[A]` marker.


```

websocat 1.11.0
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Command-line client for web sockets, like netcat/curl/socat for ws://.

USAGE:
    websocat ws://URL | wss://URL               (simple client)
    websocat -s port                            (simple server)
    websocat [FLAGS] [OPTIONS] <addr1> <addr2>  (advanced mode)

FLAGS:
        --stdout-announce-listening-ports       [A] Print a line to stdout for each port being listened
        --async-stdio                           [A] On UNIX, set stdin and stdout to nonblocking mode instead of
                                                spawning a thread. This should improve performance, but may break other
                                                programs running on the same console.
        --compress-deflate                      [A] Compress data coming to a WebSocket using deflate method. Affects
                                                only binary WebSocket messages.
        --compress-gzip                         [A] Compress data coming to a WebSocket using gzip method. Affects only
                                                binary WebSocket messages.
        --compress-zlib                         [A] Compress data coming to a WebSocket using zlib method. Affects only
                                                binary WebSocket messages.
        --crypto-reverse                        [A] Swap encryption and decryption operations in `crypto:` specifier -
                                                encrypt on read, decrypto on write.
        --dump-spec                             [A] Instead of running, dump the specifiers representation to stdout
    -e, --set-environment                       Set WEBSOCAT_* environment variables when doing exec:/cmd:/sh-c:
                                                Currently it's WEBSOCAT_URI and WEBSOCAT_CLIENT for
                                                request URI and client address (if TCP)
                                                Beware of ShellShock or similar security problems.
    -E, --exit-on-eof                           Close a data transfer direction if the other one reached EOF
        --foreachmsg-wait-read                  [A] Wait for reading to finish before closing foreachmsg:'s peer
        --jsonrpc                               Format messages you type as JSON RPC 2.0 method calls. First word
                                                becomes method name, the rest becomes parameters, possibly automatically
                                                wrapped in [].
        --just-generate-key                     [A] Just a Sec-WebSocket-Key value without running main Websocat
        --linemode-strip-newlines               [A] Don't include trailing \n or \r\n coming from streams in WebSocket
                                                messages
    -0, --null-terminated                       Use \0 instead of \n for linemode
        --no-line                               [A] Don't automatically insert line-to-message transformation
        --no-exit-on-zeromsg                    [A] Don't exit when encountered a zero message. Zero messages are used
                                                internally in Websocat, so it may fail to close connection at all.
        --no-fixups                             [A] Don't perform automatic command-line fixups. May destabilize
                                                websocat operation. Use --dump-spec without --no-fixups to discover what
                                                is being inserted automatically and read the full manual about Websocat
                                                internal workings.
        --no-async-stdio                        [A] Inhibit using stdin/stdout in a nonblocking way if it is not a tty
    -1, --one-message                           Send and/or receive only one message. Use with --no-close and/or -u/-U.
        --oneshot                               Serve only once. Not to be confused with -1 (--one-message)
        --print-ping-rtts                       Print measured round-trip-time to stderr after each received WebSocket
                                                pong.
        --exec-sighup-on-stdin-close            [A] Make exec: or sh-c: or cmd: send SIGHUP on UNIX when input is
                                                closed.
        --exec-sighup-on-zero-msg               [A] Make exec: or sh-c: or cmd: send SIGHUP on UNIX when facing incoming
                                                zero-length message.
    -q                                          Suppress all diagnostic messages, except of startup errors
        --reuser-send-zero-msg-on-disconnect    [A] Make reuse-raw: send a zero-length message to the peer when some
                                                clients disconnects.
    -s, --server-mode                           Simple server mode: specify TCP port or addr:port as single argument
    -S, --strict                                strict line/message mode: drop too long messages instead of splitting
                                                them, drop incomplete lines.
        --timestamp-monotonic                   [A] Use monotonic clock for `timestamp:` overlay
    -k, --insecure                              Accept invalid certificates and hostnames while connecting to TLS
        --udp-broadcast                         [A] Set SO_BROADCAST
        --udp-multicast-loop                    [A] Set IP[V6]_MULTICAST_LOOP
        --udp-oneshot                           [A] udp-listen: replies only one packet per client
        --udp-reuseaddr                         [A] Set SO_REUSEADDR for UDP socket. Listening TCP sockets are always
                                                reuseaddr.
        --uncompress-deflate                    [A] Uncompress data coming from a WebSocket using deflate method.
                                                Affects only binary WebSocket messages.
        --uncompress-gzip                       [A] Uncompress data coming from a WebSocket using deflate method.
                                                Affects only binary WebSocket messages.
        --uncompress-zlib                       [A] Uncompress data coming from a WebSocket using deflate method.
                                                Affects only binary WebSocket messages.
    -u, --unidirectional                        Inhibit copying data in one direction
    -U, --unidirectional-reverse                Inhibit copying data in the other direction (or maybe in both directions
                                                if combined with -u)
        --accept-from-fd                        [A] Do not call `socket(2)` in UNIX socket listener peer, start with
                                                `accept(2)` using specified file descriptor number as argument instead
                                                of filename
        --unlink                                [A] Unlink listening UNIX socket before binding to it
    -V, --version                               Prints version information
    -v                                          Increase verbosity level to info or further
    -b, --binary                                Send message to WebSockets as binary messages
    -n, --no-close                              Don't send Close message to websocket on EOF
        --websocket-ignore-zeromsg              [A] Silently drop incoming zero-length WebSocket messages. They may
                                                cause connection close due to usage of zero-len message as EOF flag
                                                inside Websocat.
    -t, --text                                  Send message to WebSockets as text messages
        --base64                                Encode incoming binary WebSocket messages in one-line Base64 If
                                                `--binary-prefix` (see `--help=full`) is set, outgoing WebSocket
                                                messages that start with the prefix are decoded from base64 prior to
                                                sending.
        --base64-text                           [A] Encode incoming text WebSocket messages in one-line Base64. I don't
                                                know whether it can be ever useful, but it's for symmetry with
                                                `--base64`.

OPTIONS:
        --socks5 <auto_socks5>
            Use specified address:port as a SOCKS5 proxy. Note that proxy authentication is not supported yet. Example:
            --socks5 127.0.0.1:9050
        --autoreconnect-delay-millis <autoreconnect_delay_millis>
            [A] Delay before reconnect attempt for `autoreconnect:` overlay. [default: 20]

        --basic-auth <basic_auth>
            Add `Authorization: Basic` HTTP request header with this base64-encoded parameter

        --queue-len <broadcast_queue_len>
            [A] Number of pending queued messages for broadcast reuser [default: 16]

    -B, --buffer-size <buffer_size>                                  Maximum message size, in bytes [default: 65536]
        --byte-to-exit-on <byte_to_exit_on>
            [A] Override the byte which byte_to_exit_on: overlay looks for [default: 28]

        --client-pkcs12-der <client_pkcs12_der>                      [A] Client identity TLS certificate
        --client-pkcs12-passwd <client_pkcs12_passwd>
            [A] Password for --client-pkcs12-der pkcs12 archive. Required on Mac.

        --close-reason <close_reason>
            Close connection with a reason message. This option only takes effect if --close-status-code option is
            provided as well.
        --close-status-code <close_status_code>                      Close connection with a status code.
        --crypto-key <crypto_key>
            [A] Specify encryption/decryption key for `crypto:` specifier. Requires `base64:`, `file:` or `pwd:` prefix.

    -H, --header <custom_headers>...
            Add custom HTTP header to websocket client request. Separate header name and value with a colon and
            optionally a single space. Can be used multiple times. Note that single -H may eat multiple further
            arguments, leading to confusing errors. Specify headers at the end or with equal sign like -H='X: y'.
        --server-header <custom_reply_headers>...
            Add custom HTTP header to websocket upgrade reply. Separate header name and value with a colon and
            optionally a single space. Can be used multiple times. Note that single -H may eat multiple further
            arguments, leading to confusing errors.
        --exec-args <exec_args>...
            [A] Arguments for the `exec:` specifier. Must be the last option, everything after it gets into the exec
            args list.
        --header-to-env <headers_to_env>...
            Forward specified incoming request header to H_* environment variable for `exec:`-like specifiers.

    -h, --help <help>
            See the help.
            --help=short is the list of easy options and address types
            --help=long lists all options and types (see [A] markers)
            --help=doc also shows longer description and examples.
        --just-generate-accept <just_generate_accept>
            [A] Just a Sec-WebSocket-Accept value based on supplied Sec-WebSocket-Key value without running main
            Websocat
        --max-messages <max_messages>
            Maximum number of messages to copy in one direction.

        --max-messages-rev <max_messages_rev>
            Maximum number of messages to copy in the other direction.

        --conncap <max_parallel_conns>
            Maximum number of simultaneous connections for listening mode

        --max-ws-frame-length <max_ws_frame_length>
            [A] Maximum size of incoming WebSocket frames, to prevent memory overflow [default: 104857600]

        --max-ws-message-length <max_ws_message_length>
            [A] Maximum size of incoming WebSocket messages (sans of one data frame), to prevent memory overflow
            [default: 209715200]
        --origin <origin>                                            Add Origin HTTP header to websocket client request
        --pkcs12-der <pkcs12_der>
            Pkcs12 archive needed to accept SSL connections, certificate and key.
            A command to output it: openssl pkcs12 -export -out output.pkcs12 -inkey key.pem -in cert.pem
            Use with -s (--server-mode) option or with manually specified TLS overlays.
            See moreexamples.md for more info.
        --pkcs12-passwd <pkcs12_passwd>
            Password for --pkcs12-der pkcs12 archive. Required on Mac.

    -p, --preamble <preamble>...
            Prepend copied data with a specified string. Can be specified multiple times.

    -P, --preamble-reverse <preamble_reverse>...
            Prepend copied data with a specified string (reverse direction). Can be specified multiple times.

        --prometheus <prometheus>
            Expose Prometheus metrics on specified IP address and port in addition to running usual Websocat session

        --request-header <request_headers>...
            [A] Specify HTTP request headers for `http-request:` specifier.

    -X, --request-method <request_method>                            [A] Method to use for `http-request:` specifier
        --request-uri <request_uri>                                  [A] URI to use for `http-request:` specifier
        --restrict-uri <restrict_uri>
            When serving a websocket, only accept the given URI, like `/ws`
            This liberates other URIs for things like serving static files or proxying.
    -F, --static-file <serve_static_files>...
            Serve a named static file for non-websocket connections.
            Argument syntax: <URI>:<Content-Type>:<file-path>
            Argument example: /index.html:text/html:index.html
            Directories are not and will not be supported for security reasons.
            Can be specified multiple times. Recommended to specify them at the end or with equal sign like `-F=...`,
            otherwise this option may eat positional arguments
        --socks5-bind-script <socks5_bind_script>
            [A] Execute specified script in `socks5-bind:` mode when remote port number becomes known.

        --socks5-destination <socks_destination>
            [A] Examples: 1.2.3.4:5678  2600:::80  hostname:5678

        --tls-domain <tls_domain>
            [A] Specify domain for SNI or certificate verification when using tls-connect: overlay

        --udp-multicast <udp_join_multicast_addr>...
            [A] Issue IP[V6]_ADD_MEMBERSHIP for specified multicast address. Can be specified multiple times.

        --udp-multicast-iface-v4 <udp_join_multicast_iface_v4>...
            [A] IPv4 address of multicast network interface. Has to be either not specified or specified the same number
            of times as multicast IPv4 addresses. Order matters.
        --udp-multicast-iface-v6 <udp_join_multicast_iface_v6>...
            [A] Index of network interface for IPv6 multicast. Has to be either not specified or specified the same
            number of times as multicast IPv6 addresses. Order matters.
        --udp-ttl <udp_ttl>                                          [A] Set IP_TTL, also IP_MULTICAST_TTL if applicable
        --protocol <websocket_protocol>
            Specify this Sec-WebSocket-Protocol: header when connecting

        --server-protocol <websocket_reply_protocol>
            Force this Sec-WebSocket-Protocol: header when accepting a connection

        --websocket-version <websocket_version>                      Override the Sec-WebSocket-Version value
        --binary-prefix <ws_binary_prefix>
            [A] Prepend specified text to each received WebSocket binary message. Also strip this prefix from outgoing
            messages, explicitly marking them as binary even if `--text` is specified
        --ws-c-uri <ws_c_uri>
            [A] URI to use for ws-c: overlay [default: ws://0.0.0.0/]

        --ping-interval <ws_ping_interval>                           Send WebSocket pings each this number of seconds
        --ping-timeout <ws_ping_timeout>
            Drop WebSocket connection if Pong message not received for this number of seconds

        --text-prefix <ws_text_prefix>
            [A] Prepend specified text to each received WebSocket text message. Also strip this prefix from outgoing
            messages, explicitly marking them as text even if `--binary` is specified

ARGS:
    <addr1>    In simple mode, WebSocket URL to connect. In advanced mode first address (there are many kinds of
               addresses) to use. See --help=types for info about address types. If this is an address for
               listening, it will try serving multiple connections.
    <addr2>    In advanced mode, second address to connect. If this is an address for listening, it will accept only
               one connection.


Basic examples:
  Command-line websocket client:
    websocat ws://ws.vi-server.org/mirror/
    
  WebSocket server
    websocat -s 8080
    
  WebSocket-to-TCP proxy:
    websocat --binary ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    

```

# Full list of address types

"Advanced" address types are denoted by `[A]` marker.


### `ws://`

Internal name for --dump-spec: WsClient


Insecure (ws://) WebSocket client. Argument is host and URL.

Example: connect to public WebSocket loopback and copy binary chunks from stdin to the websocket.

    websocat - ws://echo.websocket.org/


### `wss://`

Internal name for --dump-spec: WsClientSecure


Secure (wss://) WebSocket client. Argument is host and URL.

Example: forward TCP port 4554 to a websocket

    websocat tcp-l:127.0.0.1:4554 wss://127.0.0.1/some_websocket

### `ws-listen:`

Aliases: `ws-l:`, `l-ws:`, `listen-ws:`  
Internal name for --dump-spec: WsTcpServer


WebSocket server. Argument is host and port to listen.

Example: Dump all incoming websocket data to console

    websocat ws-l:127.0.0.1:8808 -

Example: the same, but more verbose:

    websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-


### `inetd-ws:`

Aliases: `ws-inetd:`  
Internal name for --dump-spec: WsInetdServer


WebSocket inetd server. [A]

TODO: transfer the example here


### `l-ws-unix:`

Internal name for --dump-spec: WsUnixServer


WebSocket UNIX socket-based server. [A]


### `l-ws-abstract:`

Internal name for --dump-spec: WsAbstractUnixServer


WebSocket abstract-namespaced UNIX socket server. [A]


### `ws-lowlevel-client:`

Aliases: `ws-ll-client:`, `ws-ll-c:`  
Internal name for --dump-spec: WsLlClient


[A] Low-level HTTP-independent WebSocket client connection without associated HTTP upgrade.

Example: TODO


### `ws-lowlevel-server:`

Aliases: `ws-ll-server:`, `ws-ll-s:`  
Internal name for --dump-spec: WsLlServer


[A] Low-level HTTP-independent WebSocket server connection without associated HTTP upgrade.

Example: TODO


### `wss-listen:`

Aliases: `wss-l:`, `l-wss:`, `wss-listen:`  
Internal name for --dump-spec: WssListen


Listen for secure WebSocket connections on a TCP port

Example: wss:// echo server + client for testing

    websocat -E -t --pkcs12-der=q.pkcs12 wss-listen:127.0.0.1:1234 mirror:
    websocat --ws-c-uri=wss://localhost/ -t - ws-c:cmd:'socat - ssl:127.0.0.1:1234,verify=0'

See [moreexamples.md](./moreexamples.md) for info about generation of `q.pkcs12`.


### `http:`

Internal name for --dump-spec: Http


[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
the torch to outer peer, if any - highlevel version.

Content you write becomes body, content you read is body that server has sent.

URI is specified inline.

Example:

    websocat  -b - http://example.com < /dev/null


### `asyncstdio:`

Internal name for --dump-spec: AsyncStdio


[A] Set stdin and stdout to nonblocking mode, then use it as a communication counterpart. UNIX-only.
May cause problems with programs running at the same terminal. This specifier backs the `--async-stdio` CLI option. 

Typically this specifier can be specified only one time.
    
Example: simulate `cat(1)`. This is an exception from "only one time" rule above:

    websocat - -

Example: SSH transport

    ssh -c ProxyCommand='websocat asyncstdio: ws://myserver/mywebsocket' user@myserver


### `inetd:`

Internal name for --dump-spec: Inetd


Like `asyncstdio:`, but intended for inetd(8) usage. [A]

Automatically enables `-q` (`--quiet`) mode.

`inetd-ws:` - is of `ws-l:inetd:`

Example of inetd.conf line that makes it listen for websocket
connections on port 1234 and redirect the data to local SSH server.

    1234 stream tcp nowait myuser  /opt/websocat websocat inetd-ws: tcp:127.0.0.1:22


### `tcp:`

Aliases: `tcp-connect:`, `connect-tcp:`, `tcp-c:`, `c-tcp:`  
Internal name for --dump-spec: TcpConnect


Connect to specified TCP host and port. Argument is a socket address.

Example: simulate netcat netcat

    websocat - tcp:127.0.0.1:22

Example: redirect websocket connections to local SSH server over IPv6

    websocat ws-l:0.0.0.0:8084 tcp:[::1]:22


### `tcp-listen:`

Aliases: `listen-tcp:`, `tcp-l:`, `l-tcp:`  
Internal name for --dump-spec: TcpListen


Listen TCP port on specified address.
    
Example: echo server

    websocat tcp-l:0.0.0.0:1441 mirror:
    
Example: redirect TCP to a websocket

    websocat tcp-l:0.0.0.0:8088 ws://echo.websocket.org


### `ssl-listen:`

Aliases: `ssl-l:`, `tls-l:`, `tls-listen:`, `l-ssl:`, `listen-ssl:`, `listen-tls:`, `listen-tls:`  
Internal name for --dump-spec: TlsListen


Listen for SSL connections on a TCP port

Example: Non-websocket SSL echo server

    websocat -E -b --pkcs12-der=q.pkcs12 ssl-listen:127.0.0.1:1234 mirror:
    socat - ssl:127.0.0.1:1234,verify=0


### `sh-c:`

Internal name for --dump-spec: ShC


Start specified command line using `sh -c` (even on Windows)
  
Example: serve a counter

    websocat -U ws-l:127.0.0.1:8008 sh-c:'for i in 0 1 2 3 4 5 6 7 8 9 10; do echo $i; sleep 1; done'
  
Example: unauthenticated shell

    websocat --exit-on-eof ws-l:127.0.0.1:5667 sh-c:'bash -i 2>&1'


### `cmd:`

Internal name for --dump-spec: Cmd


Start specified command line using `sh -c` or `cmd /C` (depending on platform)

Otherwise should be the the same as `sh-c:` (see examples from there).


### `exec:`

Internal name for --dump-spec: Exec


Execute a program directly (without a subshell), providing array of arguments on Unix [A]

Example: Serve current date

  websocat -U ws-l:127.0.0.1:5667 exec:date
  
Example: pinger

  websocat -U ws-l:127.0.0.1:5667 exec:ping --exec-args 127.0.0.1 -c 1
  


### `readfile:`

Internal name for --dump-spec: ReadFile


Synchronously read a file. Argument is a file path.

Blocking on operations with the file pauses the whole process

Example: Serve the file once per connection, ignore all replies.

    websocat ws-l:127.0.0.1:8000 readfile:hello.json



### `writefile:`

Internal name for --dump-spec: WriteFile



Synchronously truncate and write a file.

Blocking on operations with the file pauses the whole process

Example:

    websocat ws-l:127.0.0.1:8000 writefile:data.txt



### `appendfile:`

Internal name for --dump-spec: AppendFile



Synchronously append a file.

Blocking on operations with the file pauses the whole process

Example: Logging all incoming data from WebSocket clients to one file

    websocat -u ws-l:127.0.0.1:8000 reuse:appendfile:log.txt


### `udp:`

Aliases: `udp-connect:`, `connect-udp:`, `udp-c:`, `c-udp:`  
Internal name for --dump-spec: UdpConnect


Send and receive packets to specified UDP socket, from random UDP port  


### `udp-listen:`

Aliases: `listen-udp:`, `udp-l:`, `l-udp:`  
Internal name for --dump-spec: UdpListen


Bind an UDP socket to specified host:port, receive packet
from any remote UDP socket, send replies to recently observed
remote UDP socket.

Note that it is not a multiconnect specifier like e.g. `tcp-listen`:
entire lifecycle of the UDP socket is the same connection.

File a feature request on Github if you want proper DNS-like request-reply UDP mode here.


### `open-async:`

Internal name for --dump-spec: OpenAsync


Open file for read and write and use it like a socket. [A]
Not for regular files, see readfile/writefile instead.
  
Example: Serve big blobs of random data to clients

    websocat -U ws-l:127.0.0.1:8088 open-async:/dev/urandom



### `open-fd:`

Internal name for --dump-spec: OpenFdAsync


Use specified file descriptor like a socket. [A]

Example: Serve random data to clients v2

    websocat -U ws-l:127.0.0.1:8088 reuse:open-fd:55   55< /dev/urandom


### `threadedstdio:`

Internal name for --dump-spec: ThreadedStdio


[A] Stdin/stdout, spawning a thread (threaded version).

Like `-`, but forces threaded mode instead of async mode

Use when standard input is not `epoll(7)`-able or you want to avoid setting it to nonblocking mode.


### `-`

Aliases: `stdio:`  
Internal name for --dump-spec: Stdio


Read input from console, print to console. Uses threaded implementation even on UNIX unless requested by `--async-stdio` CLI option.

Typically this specifier can be specified only one time.
    
Example: simulate `cat(1)`. This is an exception from "only one time" rule above:

    websocat - -

Example: SSH transport

    ssh -c ProxyCommand='websocat - ws://myserver/mywebsocket' user@myserver


### `unix:`

Aliases: `unix-connect:`, `connect-unix:`, `unix-c:`, `c-unix:`  
Internal name for --dump-spec: UnixConnect


Connect to UNIX socket. Argument is filesystem path. [A]

Example: forward connections from websockets to a UNIX stream socket

    websocat ws-l:127.0.0.1:8088 unix:the_socket


### `unix-listen:`

Aliases: `listen-unix:`, `unix-l:`, `l-unix:`  
Internal name for --dump-spec: UnixListen


Listen for connections on a specified UNIX socket [A]

Example: forward connections from a UNIX socket to a WebSocket

    websocat --unlink unix-l:the_socket ws://127.0.0.1:8089

Example: Accept forwarded WebSocket connections from Nginx

    umask 0000
    websocat --unlink -b -E ws-u:unix-l:/tmp/wstest tcp:[::]:22

Nginx config:

    location /ws {
        proxy_read_timeout 7d;
        proxy_send_timeout 7d;
        #proxy_pass http://localhost:3012;
        proxy_pass http://unix:/tmp/wstest;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection \"upgrade\";
    }

This configuration allows to make Nginx responsible for
SSL and also it can choose which connections to forward
to websocat based on URLs.

Obviously, Nginx can also redirect to TCP-listening
websocat just as well - UNIX sockets are not a requirement for this feature.

See `moreexamples.md` for SystemD usage (untested).

TODO: --chmod option?


### `unix-dgram:`

Internal name for --dump-spec: UnixDgram


Send packets to one path, receive from the other. [A]
A socket for sending must be already opened.

I don't know if this mode has any use, it is here just for completeness.

Example:

    socat unix-recv:./sender -&
    websocat - unix-dgram:./receiver:./sender


### `abstract:`

Aliases: `abstract-connect:`, `connect-abstract:`, `abstract-c:`, `c-abstract:`  
Internal name for --dump-spec: AbstractConnect


Connect to UNIX abstract-namespaced socket. Argument is some string used as address. [A]

Too long addresses may be silently chopped off.

Example: forward connections from websockets to an abstract stream socket

    websocat ws-l:127.0.0.1:8088 abstract:the_socket

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.


### `abstract-listen:`

Aliases: `listen-abstract:`, `abstract-l:`, `l-abstract:`  
Internal name for --dump-spec: AbstractListen


Listen for connections on a specified abstract UNIX socket [A]

Example: forward connections from an abstract UNIX socket to a WebSocket

    websocat abstract-l:the_socket ws://127.0.0.1:8089

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.


### `abstract-dgram:`

Internal name for --dump-spec: AbstractDgram


Send packets to one address, receive from the other. [A]
A socket for sending must be already opened.

I don't know if this mode has any use, it is here just for completeness.

Example (untested):

    websocat - abstract-dgram:receiver_addr:sender_addr

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them. In particular, this mode
may fail to work without `workaround1` Cargo feature.


### `mirror:`

Internal name for --dump-spec: Mirror


Simply copy output to input. No arguments needed.

Example: emulate echo.websocket.org

    websocat -t ws-l:127.0.0.1:1234 mirror:


### `literalreply:`

Internal name for --dump-spec: LiteralReply


Reply with a specified string for each input packet.

Example:

    websocat ws-l:0.0.0.0:1234 literalreply:'{"status":"OK"}'


### `clogged:`

Internal name for --dump-spec: Clogged


Do nothing. Don't read or write any bytes. Keep connections in "hung" state. [A]


### `literal:`

Internal name for --dump-spec: Literal


Output a string, discard input.

Example:

    websocat ws-l:127.0.0.1:8080 literal:'{ "hello":"world"} '


### `assert:`

Internal name for --dump-spec: Assert


Check the input.  [A]

Read entire input and panic the program if the input is not equal
to the specified string. Used in tests.


### `assert2:`

Internal name for --dump-spec: Assert2


Check the input. [A]

Read entire input and emit an error if the input is not equal
to the specified string.


### `seqpacket:`

Aliases: `seqpacket-connect:`, `connect-seqpacket:`, `seqpacket-c:`, `c-seqpacket:`  
Internal name for --dump-spec: SeqpacketConnect


Connect to AF_UNIX SOCK_SEQPACKET socket. Argument is a filesystem path. [A]

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long paths are silently truncated.

Example: forward connections from websockets to a UNIX seqpacket abstract socket

    websocat ws-l:127.0.0.1:1234 seqpacket:@test


### `seqpacket-listen:`

Aliases: `listen-seqpacket:`, `seqpacket-l:`, `l-seqpacket:`  
Internal name for --dump-spec: SeqpacketListen


Listen for connections on a specified AF_UNIX SOCK_SEQPACKET socket [A]

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long (>=108 bytes) paths are silently truncated.

Example: forward connections from a UNIX seqpacket socket to a WebSocket

    websocat --unlink seqpacket-l:the_socket ws://127.0.0.1:8089


### `random:`

Internal name for --dump-spec: Random


Generate random bytes when being read from, discard written bytes.

    websocat -b random: ws://127.0.0.1/flood





# Full list of overlays

"Advanced" overlays denoted by `[A]` marker.


### `ws-upgrade:`

Aliases: `upgrade-ws:`, `ws-u:`, `u-ws:`  
Internal name for --dump-spec: WsServer


WebSocket upgrader / raw server. Specify your own protocol instead of usual TCP. [A]

All other WebSocket server modes actually use this overlay under the hood.

Example: serve incoming connection from socat

    socat tcp-l:1234,fork,reuseaddr exec:'websocat -t ws-u\:stdio\: mirror\:'


### `http-request:`

Internal name for --dump-spec: HttpRequest


[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
the torch to outer peer, if any - lowlevel version.

Content you write becomes body, content you read is body that server has sent.

URI is specified using a separate command-line parameter

Example:

    websocat -Ub - http-request:tcp:example.com:80 --request-uri=http://example.com/ --request-header 'Connection: close'


### `http-post-sse:`

Internal name for --dump-spec: HttpPostSse


[A] Accept HTTP/1 request. Then, if it is GET,
unidirectionally return incoming messages as server-sent events (SSE).

If it is POST then, also unidirectionally, write body upstream.

Example - turn SSE+POST pair into a client WebSocket connection:

    websocat -E -t http-post-sse:tcp-l:127.0.0.1:8080 reuse:ws://127.0.0.1:80/websock

`curl -dQQQ http://127.0.0.1:8080/` would send into it and 
`curl -N http://127.0.0.1:8080/` would recv from it.


### `ssl-connect:`

Aliases: `ssl-c`, `ssl:`, `tls:`, `tls-connect:`, `c-ssl:`, `connect-ssl:`, `c-tls:`, `connect-tls:`  
Internal name for --dump-spec: TlsConnect


Overlay to add TLS encryption atop of existing connection [A]

Example: manually connect to a secure websocket

    websocat -t - ws-c:tls-c:tcp:174.129.224.73:1080 --ws-c-uri ws://echo.websocket.org --tls-domain echo.websocket.org

For a user-friendly solution, see --socks5 command-line option


### `ssl-accept:`

Aliases: `ssl-a:`, `tls-a:`, `tls-accept:`, `a-ssl:`, `accept-ssl:`, `accept-tls:`, `accept-tls:`  
Internal name for --dump-spec: TlsAccept


Accept an TLS connection using arbitrary backing stream. [A]

Example: The same as in TlsListenClass's example, but with manual acceptor

    websocat -E -b --pkcs12-der=q.pkcs12 tls-a:tcp-l:127.0.0.1:1234 mirror:


### `reuse-raw:`

Aliases: `raw-reuse:`  
Internal name for --dump-spec: Reuser


Reuse subspecifier for serving multiple clients: unpredictable mode. [A]

Better used with --unidirectional, otherwise replies get directed to
random connected client.

Example: Forward multiple parallel WebSocket connections to a single persistent TCP connection

    websocat -u ws-l:0.0.0.0:8800 reuse:tcp:127.0.0.1:4567

Example (unreliable): don't disconnect SSH when websocket reconnects

    websocat ws-l:[::]:8088 reuse:tcp:127.0.0.1:22


### `broadcast:`

Aliases: `reuse:`, `reuse-broadcast:`, `broadcast-reuse:`  
Internal name for --dump-spec: BroadcastReuser


Reuse this connection for serving multiple clients, sending replies to all clients.

Messages from any connected client get directed to inner connection,
replies from the inner connection get duplicated across all connected
clients (and are dropped if there are none).

If WebSocket client is too slow for accepting incoming data,
messages get accumulated up to the configurable --broadcast-buffer, then dropped.

Example: Simple data exchange between connected WebSocket clients

    websocat -E ws-l:0.0.0.0:8800 reuse-broadcast:mirror:


### `autoreconnect:`

Internal name for --dump-spec: AutoReconnect


Re-establish underlying connection on any error or EOF

Example: keep connecting to the port or spin 100% CPU trying if it is closed.

    websocat - autoreconnect:tcp:127.0.0.1:5445
    
Example: keep remote logging connection open (or flood the host if port is closed):

    websocat -u ws-l:0.0.0.0:8080 reuse:autoreconnect:tcp:192.168.0.3:1025
  
TODO: implement delays between reconnect attempts


### `ws-c:`

Aliases: `c-ws:`, `ws-connect:`, `connect-ws:`  
Internal name for --dump-spec: WsConnect


Low-level WebSocket connector. Argument is a some another address. [A]

URL and Host: header being sent are independent from the underlying connection.

Example: connect to echo server in more explicit way

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:tcp:174.129.224.73:80

Example: connect to echo server, observing WebSocket TCP packet exchange

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:cmd:"socat -v -x - tcp:174.129.224.73:80"



### `msg2line:`

Internal name for --dump-spec: Message2Line


Line filter: Turns messages from packet stream into lines of byte stream. [A]

Ensure each message (a chunk from one read call from underlying connection)
contains no inner newlines (or zero bytes) and terminates with one newline.

Reverse of the `line2msg:`.

Unless --null-terminated, replaces both newlines (\x0A) and carriage returns (\x0D) with spaces (\x20) for each read.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option on top of the stack containing a websocket.

Example: TODO


### `line2msg:`

Internal name for --dump-spec: Line2Message


Line filter: turn lines from byte stream into messages as delimited by '\\n' or '\\0' [A]

Ensure that each message (a successful read call) is obtained from a line [A]
coming from underlying specifier, buffering up or splitting content as needed.

Reverse of the `msg2line:`.

Does not affect writing at all. Use this specifier on both ends to get bi-directional behaviour.

Automatically inserted by --line option at the top of the stack opposite to websocket-containing stack.

Example: TODO


### `foreachmsg:`

Internal name for --dump-spec: Foreachmsg


Execute something for each incoming message.

Somewhat the reverse of the `autoreconnect:`.

Example:

    websocat -t -u ws://server/listen_for_updates foreachmsg:writefile:status.txt

This keeps only recent incoming message in file and discards earlier messages.


### `log:`

Internal name for --dump-spec: Log


Log each buffer as it pass though the underlying connector.

If you increase the logging level, you will also see hex buffers.

Example: view WebSocket handshake and traffic on the way to echo.websocket.org

    websocat -t - ws-c:log:tcp:127.0.0.1:1080 --ws-c-uri ws://echo.websocket.org



### `jsonrpc:`

Internal name for --dump-spec: JsonRpc


[A] Turns messages like `abc 1,2` into `{"jsonrpc":"2.0","id":412, "method":"abc", "params":[1,2]}`.

For simpler manual testing of websocket-based JSON-RPC services

Example: TODO


### `timestamp:`

Internal name for --dump-spec: Timestamp


[A] Prepend timestamp to each incoming message.

Example: TODO


### `socks5-connect:`

Internal name for --dump-spec: SocksProxy


SOCKS5 proxy client (raw) [A]

Example: connect to a websocket using local `ssh -D` proxy

    websocat -t - ws-c:socks5-connect:tcp:127.0.0.1:1080 --socks5-destination echo.websocket.org:80 --ws-c-uri ws://echo.websocket.org

For a user-friendly solution, see --socks5 command-line option


### `socks5-bind:`

Internal name for --dump-spec: SocksBind


SOCKS5 proxy client (raw, bind command) [A]

Example: bind to a websocket using some remote SOCKS server

    websocat -v -t ws-u:socks5-bind:tcp:132.148.129.183:14124 - --socks5-destination 255.255.255.255:65535

Note that port is typically unpredictable. Use --socks5-bind-script option to know the port.
See an example in moreexamples.md for more thorough example.


### `crypto:`

Internal name for --dump-spec: Crypto


[A] Encrypts written messages and decrypts (and verifies) read messages with a static key, using ChaCha20-Poly1305 algorithm.

Do not not use in stream mode - packet boundaries are significant.

Note that attacker may duplicate, drop or reorder messages, including between different Websocat sessions with the same key.

Each encrypted message is 12 bytes bigger than original message.

Associated --crypto-key option accepts the following prefixes:

- `file:` prefix means that Websocat should read 32-byte file and use it as a key.
- `base64:` prefix means the rest of the value is base64-encoded 32-byte buffer
- `pwd:` means Websocat should use argon2 derivation from the specified password as a key

Use `--crypto-reverse` option to swap encryption and decryption.

Note that `crypto:` specifier is absent in usual Websocat builds.
You may need to build Websocat from source code with `--features=crypto_peer` for it to be available.


### `prometheus:`

Aliases: `metrics:`  
Internal name for --dump-spec: Prometheus


[A] Account connections, messages, bytes and other data and expose Prometheus metrics on a separate port.

Not included by default, build a crate with `--features=prometheus_peer` to have it.
You can also use `--features=prometheus_peer,prometheus/process` to have additional metrics.


### `exit_on_specific_byte:`

Internal name for --dump-spec: ExitOnSpecificByte


[A] Turn specific byte into a EOF, allowing user to escape interactive Websocat session
when terminal is set to raw mode. Works only bytes read from the overlay, not on the written bytes.

Default byte is 1C which is typically triggered by Ctrl+\.

Example: `(stty raw -echo; websocat -b exit_on_specific_byte:stdio tcp:127.0.0.1:23; stty sane)`




  
### Address types or specifiers to be implemented later:

`sctp:`, `speedlimit:`, `quic:`

### Final example

Final example just for fun: wacky mode

    websocat ws-c:ws-l:ws-c:- tcp:127.0.0.1:5678
    
Connect to a websocket using stdin/stdout as a transport,
then accept a websocket connection over the previous websocket used as a transport,
then connect to a websocket using previous step as a transport,
then forward resulting connection to the TCP port.

(Exercise to the reader: manage to make it actually connect to 5678).

