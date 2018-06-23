# websocat
netcat and socat for [WebSockets](https://en.wikipedia.org/wiki/WebSocket).

[![Build Status](https://travis-ci.org/vi/websocat.svg?branch=master)](https://travis-ci.org/vi/websocat)
[![Gitter](https://badges.gitter.im/websocat.svg)](https://gitter.im/websocat/Lobby?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=body_badge)

```
websocat 1.0.0-beta
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Command-line client for web sockets. Like netcat, but for WebSockets. Designed like socat.

USAGE:
    websocat [FLAGS] [OPTIONS] <s1> <s2>

FLAGS:
        --dump-spec                   Instead of running, dump the specifiers representation to stdout
    -E, --exit-on-eof                 Close a data transfer direction if the other one reached EOF
    -h, --help                        Prints help information
    -l, --line                        Make each WebSocket message correspond to one line
        --linemode-retain-newlines    In --line mode, don't chop off trailing \n from messages
        --long-help                   Show full help aboput specifiers and examples
    -1, --one-message                 Send and/or receive only one message. Use with --no-close and/or -u/-U.
        --oneshot                     Serve only once. Not to be confused with -1 (--one-message)
        --udp-oneshot                 udp-listen: replies only one packet per client
    -u, --unidirectional              Inhibit copying data from right specifier to left
    -U, --unidirectional-reverse      Inhibit copying data from left specifier to right
        --unlink                      Unlink listening UNIX socket before binding to it
    -V, --version                     Prints version information
    -n, --no-close                    Don't send Close message to websocket on EOF
    -t, --text                        Send text WebSocket messages instead of binary

OPTIONS:
    -H, --header <custom_headers>...
            Add custom HTTP header to websocket client request. Separate header name and value with a colon and
            optionally a single space. Can be used multiple times.
        --exec-args <exec_args>...
            Arguments for the `exec:` specifier. Must be the last option, everything after it gets into the exec args
            list.
        --origin <origin>                          Add Origin HTTP header to websocket client request
        --protocol <websocket_protocol>            Specify Sec-WebSocket-Protocol: header
        --websocket-version <websocket_version>    Override the Sec-WebSocket-Version value
        --ws-c-uri <ws_c_uri>                      URI to use for ws-c: specifier [default: ws://0.0.0.0/]



ARGS:
    <s1>    First, listening/connecting specifier. See --long-help for info about specifiers.
    <s2>    Second, connecting specifier


Basic examples:
  Connect stdin/stdout to a websocket:
    websocat - ws://echo.websocket.org/
    
  Listen websocket and redirect it to a TCP port:
    websocat ws-l:127.0.0.1:8080 tcp:127.0.0.1:5678
    
  See more examples with the --long-help option
  
Short list of specifiers (see --long-help):
  ws:// wss:// - inetd: ws-listen: inetd-ws: tcp: tcp-l: ws-c:
  autoreconnect: reuse: mirror: threadedstdio: clogged:
  literal: literalreply: assert: udp-connect: open-async:
  readfile: writefile: open-fd: unix-connect: unix-listen:
  unix-dgram: abstract-connect: abstract-listen:
  exec: sh-c:
```

It runs singlethreaded. There is old non-async threaded version in `legacy` branch of releases prior to 0.5.

Specify listening part first, unless you want websocat to serve once (like in `--oneshot` mode).

IPv6 supported, just use specifiers like `ws-l:[::1]:4567`

Web socket usage is not obligatory, you can use any specs on both sides.

If you want `wss://` server, use socat or nginx in addition to websocat until this function is implemented properly.

Pre-built binaries for Linux (usual and musl), Windows, OS X and Android (ARM) are available on the [releases page](https://github.com/vi/websocat/releases). Most are built without SSL support, so can't connect to secure `wss://` websockets, only `ws://`.

Limitations
---

* Replies to WebSocket pings are not tested at all
* Server for `wss://` is not implemented (you can workaround it with Nginx or socat).

Full list of specifiers with examples
---

(available as `--long-help`)


### WsClient

* `ws://`, `wss://`

WebSocket client. Argument is host and URL.

Example: manually interact with a web socket

    websocat - ws://echo.websocket.org/

Example: forward TCP port 4554 to a websocket

    websocat tcp-l:127.0.0.1:4554 wss://127.0.0.1/some_websocket

### WsServer

* `ws-l:`, `l-ws:`, `ws-listen:`, `listen-ws:`

WebSocket server. Argument is either IPv4 host and port to listen
or a subspecifier.

Example: Dump all incoming websocket data to console

    websocat ws-l:127.0.0.1:8808 -

Example: the same, but more verbose:

    websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-TODO


### Stdio

* `-`, `stdio:`, `inetd:`

Read input from console, print to console.

This specifier can be specified only one time.
    
When `inetd:` form is used, it also disables logging to stderr (TODO)
    
Example: simulate `cat(1)`.

    websocat - -

Example: SSH transport

    ssh -c ProxyCommand='websocat - ws://myserver/mywebsocket' user@myserver
  
`inetd-ws:` - is of `ws-l:inetd:`

Example of inetd.conf line that makes it listen for websocket
connections on port 1234 and redirect the data to local SSH server.

    1234 stream tcp nowait myuser  /opt/websocat websocat inetd-ws: tcp:127.0.0.1:22


### TcpConnect

* `tcp:`, `tcp-connect:`, `connect-tcp:`, `tcp-c:`, `c-tcp:`

Connect to specified TCP host and port. Argument is a socket address.

Example: simulate netcat netcat

    websocat - tcp:127.0.0.1:22

Example: redirect websocket connections to local SSH server over IPv6

    websocat ws-l:0.0.0.0:8084 tcp:[::1]:22


### TcpListen

* `tcp-listen:`, `listen-tcp:`, `tcp-l:`, `l-tcp:`

Listen TCP port on specified address.
    
Example: echo server

    websocat tcp-l:0.0.0.0:1441 mirror:
    
Example: redirect TCP to a websocket

    websocat tcp-l:0.0.0.0:8088 ws://echo.websocket.org


### ShC

* `sh-c:`, `cmd:`

Start specified command line using `sh -c` or `cmd /C`
  
Example: serve a counter

    websocat -U ws-l:127.0.0.1:8008 cmd:'for i in 0 1 2 3 4 5 6 7 8 9 10; do echo $i; sleep 1; done'
  
Example: unauthenticated shell

    websocat --exit-on-eof ws-l:127.0.0.1:5667 sh-c:'bash -i 2>&1'



### Exec

* `exec:`

Execute a program directly (without a subshell), providing array of arguments on Unix

Example: Serve current date

  websocat -U ws-l:127.0.0.1:5667 exec:date
  
Example: pinger

  websocat -U ws-l:127.0.0.1:5667 exec:ping --exec-args 127.0.0.1 -c 1
  


### ReadFile

* `readfile:`

Synchronously read a file. Argumen is a file path.

Blocking on operations with the file pauses the whole process

Example: Serve the file once per connection, ignore all replies.

    websocat ws-l:127.0.0.1:8000 readfile:hello.json



### WriteFile

* `writefile:`


Synchronously truncate and write a file.

Blocking on operations with the file pauses the whole process

Example:

    websocat ws-l:127.0.0.1:8000 writefile:data.txt



### AppendFile

* `appendfile:`


Synchronously append a file.

Blocking on operations with the file pauses the whole process

Example: Logging all incoming data from WebSocket clients to one file

    websocat -u ws-l:127.0.0.1:8000 reuse:appendfile:log.txt


### Reuser

* `reuse:`

Reuse subspecifier for serving multiple clients.

Better used with --unidirectional, otherwise replies get directed to
random connected client.

Example: Forward multiple parallel WebSocket connections to a single persistent TCP connection

    websocat -u ws-l:0.0.0.0:8800 reuse:tcp:127.0.0.1:4567

Example (unreliable): don't disconnect SSH when websocket reconnects

    websocat ws-l:[::]:8088 reuse:tcp:127.0.0.1:22


### AutoReconnect

* `autoreconnect:`

Re-establish underlying specifier on any error or EOF

Example: keep connecting to the port or spin 100% CPU trying if it is closed.

    websocat - autoreconnect:tcp:127.0.0.1:5445
    
Example: keep remote logging connection open (or flood the host if port is closed):

    websocat -u ws-l:0.0.0.0:8080 reuse:autoreconnect:tcp:192.168.0.3:1025
  
TODO: implement delays between reconnect attempts


### WsConnect

* `ws-c:`, `c-ws:`, `ws-connect:`, `connect-ws:`

Low-level WebSocket connector. Argument is a subspecifier.

URL and Host: header being sent are independent from the underlying specifier.

Example: connect to echo server in more explicit way

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:tcp:174.129.224.73:80

Example: connect to echo server, observing WebSocket TCP packet exchange

    websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:cmd:"socat -v -x - tcp:174.129.224.73:80"



### UdpConnect

* `udp:`, `udp-connect:`, `connect-udp:`, `udp-c:`, `c-udp:`

Send and receive packets to specified UDP socket, from random UDP port  


### UdpListen

* `udp-listen:`, `listen-udp:`, `udp-l:`, `l-udp:`

Bind an UDP socket to specifier host:port, receive packet
from any remote UDP socket, send replies to recently observed
remote UDP socket.

Note that it is not a multiconnect specifier like e.g. `tcp-listen`:
entire lifecycle of the UDP socket is the same connection.

File a feature request on Github if you want proper DNS-like request-reply UDP mode here.


### OpenAsync

* `open-async:`

Open file for read and write and use it like a socket.
Not for regular files, see readfile/writefile instead.
  
Example: Serve big blobs of random data to clients

    websocat -U ws-l:127.0.0.1:8088 open-async:/dev/urandom



### OpenFdAsync

* `open-fd:`

Use specified file descriptor like a socket

Example: Serve random data to clients v2

    websocat -U ws-l:127.0.0.1:8088 reuse:open-fd:55   55< /dev/urandom


### ThreadedStdio

* `threadedstdio:`

Stdin/stdout, spawning a thread.

Like `-`, but forces threaded mode instead of async mode

Use when standard input is not `epoll(7)`-able or you want to avoid setting it to nonblocking mode.


### UnixConnect

* `unix:`, `unix-connect:`, `connect-unix:`, `unix-c:`, `c-unix:`

Connect to UNIX socket. Argument is filesystem path.

Example: forward connections from websockets to a UNIX stream socket

    websocat ws-l:127.0.0.1:8088 unix:the_socket


### UnixListen

* `unix-listen:`, `listen-unix:`, `unix-l:`, `l-unix:`

Listen for connections on a specified UNIX socket

Example: forward connections from a UNIX socket to a WebSocket

    websocat --unlink unix-l:the_socket ws://127.0.0.1:8089
    
Example: Accept forwarded WebSocket connections from Nginx

    umask 0000
    websocat --unlink ws-l:unix-l:/tmp/wstest tcp:[::]:22
      
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

TODO: --chmod option?


### UnixDgram

* `unix-dgram:`

Send packets to one path, receive from the other.
A socket for sending must be already openend.

I don't know if this mode has any use, it is here just for completeness.

Example:

    socat unix-recv:./sender -&
    websocat - unix-dgram:./receiver:./sender


### AbstractConnect

* `abstract:`, `abstract-connect:`, `connect-abstract:`, `abstract-c:`, `c-abstract:`

Connect to UNIX abstract-namespaced socket. Argument is some string used as address.

Too long addresses may be silently chopped off.

Example: forward connections from websockets to an abstract stream socket

    websocat ws-l:127.0.0.1:8088 abstract:the_socket

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.


### AbstractListen

* `abstract-listen:`, `listen-abstract:`, `abstract-l:`, `l-abstract:`

Listen for connections on a specified abstract UNIX socket

Example: forward connections from an abstract UNIX socket to a WebSocket

    websocat abstract-l:the_socket ws://127.0.0.1:8089

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them.


### AbstractDgram

* `abstract-dgram:`

Send packets to one address, receive from the other.
A socket for sending must be already openend.

I don't know if this mode has any use, it is here just for completeness.

Example (untested):

    websocat - abstract-dgram:receiver_addr:sender_addr

Note that abstract-namespaced Linux sockets may not be normally supported by Rust,
so non-prebuilt versions may have problems with them. In particular, this mode
may fail to work without `workaround1` Cargo feature.


### Mirror

* `mirror:`

Simply copy output to input. No arguments needed.

Similar to `exec:cat`.


### LiteralReply

* `literalreply:`

Reply with a specified string for each input packet.

Example:

    websocat ws-l:0.0.0.0:1234 literalreply:'{"status":"OK"}'


### Clogged

* `clogged:`

Do nothing. Don't read or write any bytes. Keep connections in "hung" state.


### Literal

* `literal:`

Output a string, discard input.

Example:

    websocat ws-l:127.0.0.1:8080 literal:'{ "hello":"world"} '


### Assert

* `assert:`

Check the input. Read entire input and panic the program if the input is not equal
to the specified string. Used in tests.


### SeqpacketConnect

* `seqpacket:`, `seqpacket-connect:`, `connect-seqpacket:`, `seqpacket-c:`, `c-seqpacket:`

Connect to AF_UNIX SOCK_SEQPACKET socket. Argument is a filesystem path.

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long paths are silently truncated.

Example: forward connections from websockets to a UNIX seqpacket abstract socket

    websocat ws-l:127.0.0.1:1234 seqpacket:@test


### SeqpacketListen

* `seqpacket-listen:`, `listen-seqpacket:`, `seqpacket-l:`, `l-seqpacket:`

Listen for connections on a specified AF_UNIX SOCK_SEQPACKET socket

Start the path with `@` character to make it connect to abstract-namespaced socket instead.

Too long (>=108 bytes) paths are silently truncated.

Example: forward connections from a UNIX seqpacket socket to a WebSocket

    websocat --unlink seqpacket-l:the_socket ws://127.0.0.1:8089



Planned features
---

* Driving SSL server websockets (specifying cert and key)
* Pure Rust version with SSL support?
* SOCK_SEQPACKET mode for `exec:`?
* SOCK_SEQPACKET SCTP mode?
* Option for auto `\n` insertion
* Add WebRTC's DataChannel to the mix (separate project)?

There are also checkboxes on [Issues](https://github.com/vi/websocat/issues/1).

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
* [wscat](https://github.com/websockets/wscat)
* [websocketd](https://github.com/joewalnes/websocketd)
