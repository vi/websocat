# websocat
Netcat, curl and socat for [WebSockets](https://en.wikipedia.org/wiki/WebSocket).

[![Build Status](https://travis-ci.org/vi/websocat.svg?branch=master)](https://travis-ci.org/vi/websocat)
[![Gitter](https://badges.gitter.im/websocat.svg)](https://gitter.im/websocat/Lobby?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=body_badge)

## Examples

### Connect to public echo server

```
$ websocat ws://echo.websocket.org
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
$ echo 'Page.navigate {"url":"https://example.com"}' | websocat -n1 --jsonrpc ws://127.0.0.1:9222/devtools/page/A331E56CCB8615EB4FCB720425A82259
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
* Buildable by Rust starting from v1.34. Websocats up to version 1.5.0 are buildable by Rust 1.31. Websocat version 1.2.0 (or from [a tag](https://github.com/vi/websocat/releases/tag/last_version_that_supports_rust_1_28)) should be buildable by 1.28.0. Websocat version 1.1.0 and earlier should be buildable with Rust v1.23.0. There is legacy non-async version buildable by even older Rust.

[releases]:https://github.com/vi/websocat/releases

# Installation

There are multiple options for installing WebSocat. From easy to hard:

* If you're on Linux Debian or Ubuntu (or other dpkg-based), try downloading a pre-build deb package from [GitHub releases][releases] and install from GUI or with command like `gdebi websocat_..._.deb`
* If you're on Fedora, you can install WebSocat from [Copr](https://copr.fedorainfracloud.org/coprs/atim/websocat/): `sudo dnf copr enable atim/websocat -y && sudo dnf install websocat`
* If you're on FreeBSD, you may install WebSocat with the following command: `pkg install websocat`.
* If you're on Mac, you can do `brew install websocat`.
* Download a pre-build executable and install it to PATH.
* Install the [Rust toolchain](https://rustup.rs/) and do `cargo install --features=ssl websocat`. If something fails with a `-sys` crate, try without `--features=ssl`;
* Build Websocat from source code (see below), then move `target/release/websocat` somewhere to the PATH.

## Usage

```
websocat 1.5.0
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Command-line client for web sockets, like netcat/curl/socat for ws://.

USAGE:
    websocat ws://URL | wss://URL               (simple client)
    websocat -s port                            (simple server)
    websocat [FLAGS] [OPTIONS] <addr1> <addr2>  (advanced mode)

FLAGS:
    (some flags are hidden, see --help=long)
    -e, --set-environment                       Set WEBSOCAT_* environment variables when doing exec:/cmd:/sh-c:
                                                Currently it's WEBSOCAT_URI and WEBSOCAT_CLIENT for
                                                request URI and client address (if TCP)
                                                Beware of ShellShock or similar security problems.
    -E, --exit-on-eof                           Close a data transfer direction if the other one reached EOF
        --jsonrpc                               Format messages you type as JSON RPC 2.0 method calls. First word
                                                becomes method name, the rest becomes parameters, possibly automatically
                                                wrapped in [].
    -0, --null-terminated                       Use \0 instead of \n for linemode
    -1, --one-message                           Send and/or receive only one message. Use with --no-close and/or -u/-U.
        --oneshot                               Serve only once. Not to be confused with -1 (--one-message)
    -q                                          Suppress all diagnostic messages, except of startup errors
    -s, --server-mode                           Simple server mode: specify TCP port or addr:port as single argument
    -S, --strict                                strict line/message mode: drop too long messages instead of splitting
                                                them, drop incomplete lines.
    -k, --insecure                              Accept invalid certificates and hostnames while connecting to TLS
    -u, --unidirectional                        Inhibit copying data in one direction
    -U, --unidirectional-reverse                Inhibit copying data in the other direction (or maybe in both directions
                                                if combined with -u)
    -v                                          Increase verbosity level to info or further
    -b, --binary                                Send message to WebSockets as binary messages
    -n, --no-close                              Don't send Close message to websocket on EOF
    -t, --text                                  Send message to WebSockets as text messages

OPTIONS:
    (some options are hidden, see --help=long)
        --socks5 <auto_socks5>
            Use specified address:port as a SOCKS5 proxy. Note that proxy authentication is not supported yet. Example:
            --socks5 127.0.0.1:9050

    -B, --buffer-size <buffer_size>                     Maximum message size, in bytes [default: 65536]
    -H, --header <custom_headers>...
            Add custom HTTP header to websocket client request. Separate header name and value with a colon and
            optionally a single space. Can be used multiple times. Note that single -H may eat multiple further
            arguments, leading to confusing errors. Specify headers at the end or with equal sign like -H='X: y'.
        --server-header <custom_reply_headers>...
            Add custom HTTP header to websocket upgrade reply. Separate header name and value with a colon and
            optionally a single space. Can be used multiple times. Note that single -H may eat multiple further
            arguments, leading to confusing errors.
        --header-to-env <headers_to_env>...
            Forward specified incoming request header to H_* environment variable for `exec:`-like specifiers.
    -h, --help <help>
            See the help.
            --help=short is the list of easy options and address types
            --help=long lists all options and types (see [A] markers)
            --help=doc also shows longer description and examples.
        --conncap <max_parallel_conns>                  Maximum number of simultaneous connections for listening mode
        --origin <origin>                               Add Origin HTTP header to websocket client request
        --pkcs12-der <pkcs12_der>
            Pkcs12 archive needed to accept SSL connections, certificate and key.
            A command to output it: openssl pkcs12 -export -out output.pkcs12 -inkey key.pem -in cert.pem
            Use with -s (--server-mode) option or with manually specified TLS overlays.
            See moreexamples.md for more info.
        --pkcs12-passwd <pkcs12_passwd>                 Password for --pkcs12-der pkcs12 archive. Required on Mac.
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


        --protocol <websocket_protocol>                 Specify this Sec-WebSocket-Protocol: header when connecting
        --server-protocol <websocket_reply_protocol>
            Force this Sec-WebSocket-Protocol: header when accepting a connection

        --websocket-version <websocket_version>         Override the Sec-WebSocket-Version value
        --ping-interval <ws_ping_interval>              Send WebSocket pings each this number of seconds
        --ping-timeout <ws_ping_timeout>
            Drop WebSocket connection if Pong message not received for this number of seconds


ARGS:
    <addr1>    In simple mode, WebSocket URL to connect. In advanced mode first address (there are many kinds of
               addresses) to use. See --help=types for info about address types. If this is an address for
               listening, it will try serving multiple connections.
    <addr2>    In advanced mode, second address to connect. If this is an address for listening, it will accept only
               one connection.


Basic examples:
  Command-line websocket client:
    websocat ws://echo.websocket.org/
    
  WebSocket server
    websocat -s 8080
    
  WebSocket-to-TCP proxy:
    websocat --binary ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    

Partial list of address types:
	ws://           	Insecure (ws://) WebSocket client. Argument is host and URL.
	wss://          	Secure (wss://) WebSocket client. Argument is host and URL.
	ws-listen:      	WebSocket server. Argument is host and port to listen.
	wss-listen:     	Listen for secure WebSocket connections on a TCP port
	stdio:          	Same as `-`. Read input from console, print to console.
	tcp:            	Connect to specified TCP host and port. Argument is a socket address.
	tcp-listen:     	Listen TCP port on specified address.
	ssl-listen:     	Listen for SSL connections on a TCP port
	sh-c:           	Start specified command line using `sh -c` (even on Windows)
	cmd:            	Start specified command line using `sh -c` or `cmd /C` (depending on platform)
	readfile:       	Synchronously read a file. Argument is a file path.
	writefile:      	Synchronously truncate and write a file.
	appendfile:     	Synchronously append a file.
	udp:            	Send and receive packets to specified UDP socket, from random UDP port  
	udp-listen:     	Bind an UDP socket to specified host:port, receive packet
	mirror:         	Simply copy output to input. No arguments needed.
	literalreply:   	Reply with a specified string for each input packet.
	literal:        	Output a string, discard input.
Partial list of overlays:
	broadcast:      	Reuse this connection for serving multiple clients, sending replies to all clients.
	autoreconnect:  	Re-establish underlying connection on any error or EOF
```

Pre-built binaries for Linux (usual and musl), Windows, OS X and Android are available on the [releases page](https://github.com/vi/websocat/releases).

Limitations
---

* It is not convenient when text and binary WebSocket messages are mixed. This affects `mirror:` specifier, making it a bit different from ws://echo.websocket.org. There are `--binary-prefix`, `--text-prefix` and `--base64` options to handle mixture of binary and text.
* Current version of Websocat don't receive notification about closed sockets. This makes serving without `-E` or `-u` options or in backpressure scenarios prone to socket leak.

Building from source code
---

1. Install the [Rust toolchain](https://rustup.rs/)
2. `cargo build --release --features=ssl`.
3. Find the executable somewhere under `target/`, e.g. in `target/release/websocat`.


SSL on Android
---

websocat's `wss://` may fail on Android. As a workaround, download certificate bundle, for example, from https://curl.haxx.se/ca/cacert.pem and specify it explicitly:

    SSL_CERT_FILE=cacert.pem /data/local/tmp/websocat wss://echo.websocket.org

Or just use `--insecure` option.

Reference
---

There is a work-in-progress [reference document](doc.md) that contains more options and examples.

Some notes
---

* It runs singlethreaded, but can serve multiple connections simultaneously. There is old non-async threaded version in `legacy` branch of releases prior to 0.5.
* IPv6 is supported, surround your IP in square brackets or use it as is, depending on context.
* Web socket usage is not obligatory, you can use any specs on both sides.
* Typically one line in binary stream correspond to one WebSocket text message. This is adjustable with options.

Planned features
---

There are also checkboxes on issues [#1](https://github.com/vi/websocat/issues/1) and [#5](https://github.com/vi/websocat/issues/5).

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
* [wscat](https://github.com/websockets/wscat)
* [websocketd](https://github.com/joewalnes/websocketd)
* [wsd](https://github.com/alexanderGugel/wsd)
