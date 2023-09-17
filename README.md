# websocat
Netcat, curl and socat for [WebSockets](https://en.wikipedia.org/wiki/WebSocket).

[![Gitter](https://badges.gitter.im/websocat.svg)](https://gitter.im/websocat/Lobby?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=body_badge)

## Examples

### Connect to public echo server

```
$ websocat ws://ws.vi-server.org/mirror
123
123
ABC
ABC
```

### Serve and connect

```
A$ websocat -s 1234
Listening on ws://127.0.0.1:1234/
ABC
123

B$ websocat ws://127.0.0.1:1234/
ABC
123
```

### Open a tab in Chromium using remote debugging.

```
$ chromium --remote-debugging-port=9222&
$ curl -sg http://127.0.0.1:9222/json/new | grep webSocketDebuggerUrl | cut -d'"' -f4 | head -1
ws://127.0.0.1:9222/devtools/page/A331E56CCB8615EB4FCB720425A82259
$ echo 'Page.navigate {"url":"https://example.com"}' | websocat -n1 --jsonrpc --jsonrpc-omit-jsonrpc ws://127.0.0.1:9222/devtools/page/A331E56CCB8615EB4FCB720425A82259
{"id":2,"result":{"frameId":"A331E56CCB8615EB4FCB720425A82259","loaderId":"EF5AAD19F2F8BB27FAF55F94FFB27DF9"}}
```


### Proxy TCP connections to WebSocket connections and back.

```
$ websocat --oneshot -b ws-l:127.0.0.1:1234 tcp:127.0.0.1:22&
$ websocat --oneshot -b tcp-l:127.0.0.1:1236 ws://127.0.0.1:1234/&
$ nc 127.0.0.1 1236
SSH-2.0-OpenSSH_7.4p1 Debian-10+deb9u3
qwertyu
Protocol mismatch.
```


### Broadcast all messages between connected WebSocket clients

```
A$ websocat -t ws-l:127.0.0.1:1234 broadcast:mirror:
B$ websocat ws://127.0.0.1:1234
C$ websocat ws://127.0.0.1:1234
```

(if you like this one, you may actually want https://github.com/vi/wsbroad/ instead)

See [moreexamples.md](./moreexamples.md) for further examples.

## Features

* Connecting to and serving WebSockets from command line.
* Executing external program and making it communicate to WebSocket using stdin/stdout.
* Text and binary modes, converting between lines (or null-terminated records) and messages.
* Inetd mode, UNIX sockets (including abstract namespaced on Linux).
* Integration with Nginx using TCP or UNIX sockets.
* Directly using unauthenticated SOCKS5 servers for connecting to WebSockets and listening WebSocket connection.
* Auto-reconnect and connection-reuse modes.
* Linux, Windows and Mac support, with [pre-built executables][releases].
* Low-level WebSocket clients and servers with overridable underlying transport connection, e.g. calling external program to serve as a transport for websocat (for SSL, proxying, etc.).

[releases]:https://github.com/vi/websocat/releases

# Installation

There are multiple options for installing WebSocat. From easy to hard:

* If you're on Fedora, you can install WebSocat from [Copr](https://copr.fedorainfracloud.org/coprs/atim/websocat/): `sudo dnf copr enable atim/websocat -y && sudo dnf install websocat`
* If you're on FreeBSD, you may install WebSocat with the following command: `pkg install websocat`.
* If you're on Linux Debian or Ubuntu (or other dpkg-based), try downloading a pre-build executable from [GitHub releases][releases]. Websocat is not yet officially packaged for Debian. Some older versions of Websocat may also have Debian package files available on Github releases.
* If you're on macOS, you can do:
  * `brew install websocat` using [Homebrew](https://brew.sh)
  * `sudo port install websocat` using [MacPorts](https://www.macports.org)
* Download a pre-build executable and install it to PATH.
* Install the [Rust toolchain](https://rustup.rs/) and do `cargo install --features=ssl websocat`. If something fails with a `-sys` crate, try without `--features=ssl`;
* Build Websocat from source code (see below), then move `target/release/websocat` somewhere to the PATH.

Pre-built binaries for Linux (usual and musl), Windows, OS X and Android are available on the [releases page](https://github.com/vi/websocat/releases).


Building from source code
---

1. Install the [Rust toolchain](https://rustup.rs/). Note that Websocat v1 (i.e. the current stable version) may fail to support too new Rust due to its old dependencies which can be broken by e.g. [this](https://github.com/rust-lang/rust/pull/78802).
2. `cargo build --release --features=ssl`.
3. Find the executable somewhere under `target/`, e.g. in `target/release/websocat`.

### Rust versions


|Websocat versions|Minimal Rust version|Maximal Rust version|
|----|----|----|
| 1.9 - 1.11| 1.46 | maybe 1.63 |
| 1.6 - 1.8 | 1.34 | maybe 1.63  |
| 1.3 - 1.5 | 1.31 | 1.47? |
| 1.2       | 1.28 | 1.47? |
| 1.0-1.1   | 1.23 | 1.47? |
| 1.2       | ?    | ?     |

Note that building with legacy Rust version (e.g. 1.46) may require manually copying `Cargo.lock.legacy` to `Cargo.lock` prior to the building.

Early non-async versions of Websocat should be buildable by even older rustc.  

Note that old versions of Websocat may misbehave if built by Rust 1.48 or later due to https://github.com/rust-lang/rust/pull/71274/.

It may be not a good idea to build v1.x line of Websocat with Rust 1.64 due to [IP address representation refactor]. It may expose previously hidden undefined behaviour in legacy dependencies. (In practice, it seems to just work though - but a lot of time passed since I checked Websocat properly and in detail).

[ipaddr]:https://github.com/rust-lang/rust/pull/78802


SSL on Android
---

websocat's `wss://` may fail on Android. As a workaround, download certificate bundle, for example, from https://curl.haxx.se/ca/cacert.pem and specify it explicitly:

    SSL_CERT_FILE=cacert.pem /data/local/tmp/websocat wss://echo.websocket.org

Or just use `--insecure` option.

Documentation
---

Basic usage examples are provided at the top of this README and in `--help` message. More tricks are described in [moreexamples.md](./moreexamples.md).

There is a [list of all address types and overlays](doc.md).

<details><summary>`websocat --help=long` output</summary>

```
websocat 1.12.0
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
        --jsonrpc-omit-jsonrpc                  [A] Omit `jsonrpc` field when using `--jsonrpc`, e.g. for Chromium
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
        --exec-exit-on-disconnect               [A] Make exec: or sh-c: or cmd: immediately exit when connection is
                                                closed, don't wait for termination.
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
        --inhibit-pongs <inhibit_pongs>
            [A] Stop replying to incoming WebSocket pings after specified number of replies

        --just-generate-accept <just_generate_accept>
            [A] Just a Sec-WebSocket-Accept value based on supplied Sec-WebSocket-Key value without running main
            Websocat
        --max-messages <max_messages>
            Maximum number of messages to copy in one direction.

        --max-messages-rev <max_messages_rev>
            Maximum number of messages to copy in the other direction.

        --conncap <max_parallel_conns>
            Maximum number of simultaneous connections for listening mode

        --max-sent-pings <max_sent_pings>
            [A] Stop sending pings after this number of sent pings

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
    

Full list of address types:
	ws://           	Insecure (ws://) WebSocket client. Argument is host and URL.
	wss://          	Secure (wss://) WebSocket client. Argument is host and URL.
	ws-listen:      	WebSocket server. Argument is host and port to listen.
	inetd-ws:       	WebSocket inetd server. [A]
	l-ws-unix:      	WebSocket UNIX socket-based server. [A]
	l-ws-abstract:  	WebSocket abstract-namespaced UNIX socket server. [A]
	ws-lowlevel-client:	[A] Low-level HTTP-independent WebSocket client connection without associated HTTP upgrade.
	ws-lowlevel-server:	[A] Low-level HTTP-independent WebSocket server connection without associated HTTP upgrade.
	wss-listen:     	Listen for secure WebSocket connections on a TCP port
	http:           	[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
	asyncstdio:     	[A] Set stdin and stdout to nonblocking mode, then use it as a communication counterpart. UNIX-only.
	inetd:          	Like `asyncstdio:`, but intended for inetd(8) usage. [A]
	tcp:            	Connect to specified TCP host and port. Argument is a socket address.
	tcp-listen:     	Listen TCP port on specified address.
	ssl-listen:     	Listen for SSL connections on a TCP port
	sh-c:           	Start specified command line using `sh -c` (even on Windows)
	cmd:            	Start specified command line using `sh -c` or `cmd /C` (depending on platform)
	exec:           	Execute a program directly (without a subshell), providing array of arguments on Unix [A]
	readfile:       	Synchronously read a file. Argument is a file path.
	writefile:      	Synchronously truncate and write a file.
	appendfile:     	Synchronously append a file.
	udp:            	Send and receive packets to specified UDP socket, from random UDP port  
	udp-listen:     	Bind an UDP socket to specified host:port, receive packet
	open-async:     	Open file for read and write and use it like a socket. [A]
	open-fd:        	Use specified file descriptor like a socket. [A]
	threadedstdio:  	[A] Stdin/stdout, spawning a thread (threaded version).
	-               	Read input from console, print to console. Uses threaded implementation even on UNIX unless requested by `--async-stdio` CLI option.
	unix:           	Connect to UNIX socket. Argument is filesystem path. [A]
	unix-listen:    	Listen for connections on a specified UNIX socket [A]
	unix-dgram:     	Send packets to one path, receive from the other. [A]
	abstract:       	Connect to UNIX abstract-namespaced socket. Argument is some string used as address. [A]
	abstract-listen:	Listen for connections on a specified abstract UNIX socket [A]
	abstract-dgram: 	Send packets to one address, receive from the other. [A]
	mirror:         	Simply copy output to input. No arguments needed.
	literalreply:   	Reply with a specified string for each input packet.
	clogged:        	Do nothing. Don't read or write any bytes. Keep connections in "hung" state. [A]
	literal:        	Output a string, discard input.
	assert:         	Check the input.  [A]
	assert2:        	Check the input. [A]
	seqpacket:      	Connect to AF_UNIX SOCK_SEQPACKET socket. Argument is a filesystem path. [A]
	seqpacket-listen:	Listen for connections on a specified AF_UNIX SOCK_SEQPACKET socket [A]
	random:         	Generate random bytes when being read from, discard written bytes.
Full list of overlays:
	ws-upgrade:     	WebSocket upgrader / raw server. Specify your own protocol instead of usual TCP. [A]
	http-request:   	[A] Issue HTTP request, receive a 1xx or 2xx reply, then pass
	http-post-sse:  	[A] Accept HTTP/1 request. Then, if it is GET,
	ssl-connect:    	Overlay to add TLS encryption atop of existing connection [A]
	ssl-accept:     	Accept an TLS connection using arbitrary backing stream. [A]
	reuse-raw:      	Reuse subspecifier for serving multiple clients: unpredictable mode. [A]
	broadcast:      	Reuse this connection for serving multiple clients, sending replies to all clients.
	autoreconnect:  	Re-establish underlying connection on any error or EOF
	ws-c:           	Low-level WebSocket connector. Argument is a some another address. [A]
	msg2line:       	Line filter: Turns messages from packet stream into lines of byte stream. [A]
	line2msg:       	Line filter: turn lines from byte stream into messages as delimited by '\\n' or '\\0' [A]
	foreachmsg:     	Execute something for each incoming message.
	log:            	Log each buffer as it pass though the underlying connector.
	jsonrpc:        	[A] Turns messages like `abc 1,2` into `{"jsonrpc":"2.0","id":412, "method":"abc", "params":[1,2]}`.
	timestamp:      	[A] Prepend timestamp to each incoming message.
	socks5-connect: 	SOCKS5 proxy client (raw) [A]
	socks5-bind:    	SOCKS5 proxy client (raw, bind command) [A]
	crypto:         	[A] Encrypts written messages and decrypts (and verifies) read messages with a static key, using ChaCha20-Poly1305 algorithm.
	prometheus:     	[A] Account connections, messages, bytes and other data and expose Prometheus metrics on a separate port.
	exit_on_specific_byte:	[A] Turn specific byte into a EOF, allowing user to escape interactive Websocat session
```
</details>


Some notes
---

* IPv6 is supported, surround your IP in square brackets or use it as is, depending on context.
* Web socket usage is not obligatory, you can use any specs on both sides.
* Typically one line in binary stream correspond to one WebSocket text message. This is adjustable with options.

Limitations
---

* It is not convenient when text and binary WebSocket messages are mixed. This affects `mirror:` specifier, making it a bit different from ws://echo.websocket.org. There are `--binary-prefix`, `--text-prefix` and `--base64` options to handle mixture of binary and text.
* Current version of Websocat don't receive notification about closed sockets. This makes serving without `-E` or `-u` options or in backpressure scenarios prone to socket leak.
* Readline is not integrated. Users are advices to wrap websocat using [`rlwrap`](https://linux.die.net/man/1/rlwrap) tool for more convenient CLI.
* Build process of current version of Websocat is not properly automated and is fragile.
* `ws://localhost` may fail if service is not listening both IPv4 and IPv6 properly. There is a workaround based on `ws-c:tcp:` if needed. Or just use `ws://127.0.0.1`.

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
* [wscat](https://github.com/websockets/wscat)
* [websocketd](https://github.com/joewalnes/websocketd)
* [wsd](https://github.com/alexanderGugel/wsd)
