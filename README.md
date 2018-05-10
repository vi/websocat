# websocat
Websocket proxy, socat-style

```
websocat 1.0.0-alpha
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Connection forwarder from/to web sockets to/from usual sockets, in style of socat

USAGE:
    websocat [FLAGS] [OPTIONS] <s1> <s2>

FLAGS:
        --dump-spec                 Instead of running, dump the specifiers representation to stdout
        --exit-on-eof               Close a data transfer direction if the other one reached EOF
    -h, --help                      Prints help information
        --long-help                 Show full help aboput specifiers and examples
        --oneshot                   Serve only once
        --udp-oneshot               udp-listen: replies only one packet per client
    -u, --unidirectional            Inhibit copying data from right specifier to left
    -U, --unidirectional-reverse    Inhibit copying data from left specifier to right
        --unlink                    Unlink listening UNIX socket before binding to it
    -V, --version                   Prints version information
    -t, --text                      Send text WebSocket messages instead of binary

OPTIONS:
        --exec-args <exec_args>...         Arguments for the `exec:` specifier. Must be the last option, everything
                                           after it gets into the exec args list.
        --protocol <websocket_protocol>    Specify Sec-WebSocket-Protocol: header
        --ws-c-uri <ws_c_uri>              URI to use for ws-c: specifier [default: ws://0.0.0.0/]

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

Specify listening part first, unless you want websocat to serve once (like in `--oneshot` mode).

IPv6 supported, just use specifiers like `ws-l:[::1]:4567`

Web socket usage is not obligatory, you can use any specs on both sides.

If you want `wss://` server, use socat or nginx in addition to websocat until this function is implemented properly.

Pre-built binaries for Linux (usual and musl), Windows, OS X and Android (ARM) are available on the [releases page](https://github.com/vi/websocat/releases). Most are built without SSL support, so can't connect to secure `wss://` websockets, only `ws://`.

Limitations
---

* Replies to WebSocket pings are not tested at all
* Windows not tested at all
* Only partial SSL support.

Full list of specifiers
---

(available as `--long-help`)

*  `-` -- Stdin/stdout

    Read input from console, print to console.
    Can be specified only one time.
    
    Aliases: `stdio:`, `inetd:`
    
   `inetd:` also disables logging to stderr (TODO).
    
    Example: like `cat(1)`.
      
        websocat - -
      
    Example: for inetd mode
    
        websocat inetd: literal:$'Hello, world.\n'
      
    Example: SSH transport
    
        ssh -c ProxyCommand='websocat - ws://myserver/mywebsocket' user@myserver
    
*  `ws://<url>`, `wss://<url>` -- WebSocket client

    Example: forward port 4554 to a websocket
    
        websocat tcp-l:127.0.0.1:4554 wss://127.0.0.1/some_websocket
      
*  `ws-listen:<spec>` - Listen for websocket connections

    A combining specifier, but given IPv4 address as argument auto-inserts `tcp-l:`
    
    Aliases: `listen-ws:` `ws-l:` `l-ws:`
    
    Example:
    
        websocat ws-l:127.0.0.1:8808 -
    
    Example: the same, but more verbose:
    
        websocat ws-l:tcp-l:127.0.0.1:8808 reuse:-
  
*  `inetd-ws:` - Alias of `ws-l:inetd:`
  
    Example of inetd.conf line:
    
        1234 stream tcp nowait myuser  /opt/websocat websocat inetd-ws: tcp:127.0.0.1:22

  
*  `tcp:<hostport>` - connect to specified TCP host and port

    Aliases: `tcp-connect:`,`connect-tcp:`,`c-tcp:`,`tcp-c:`
    
    Example: like netcat
    
        websocat - tcp:127.0.0.1:22
      
    Example: IPv6
    
        websocat ws-l:0.0.0.0:8084 tcp:[::1]:22
    
*  `tcp-l:<hostport>` - listen TCP port on specified address
    Aliases: `l-tcp:`  `tcp-listen:` `listen-tcp:`
    
    Example: echo server
    
        websocat tcp-l:0.0.0.0:1441 mirror:
      
*  `exec:<program_path> --exec-args <arguments...> --`

    Execute a program (subprocess) directly, without a subshell.
    
    Example: date server
    
        websocat -U ws-l:127.0.0.1:5667 exec:date
      
    Example: pinger
    
        websocat -U ws-l:127.0.0.1:5667 exec:ping --exec-args 127.0.0.1 -c 1 --
  
*  `sh-c:<command line>` - start subprocess though 'sh -c' or `cmd /C`
  
    Example: unauthenticated shell
    
        websocat --exit-on-eof ws-l:127.0.0.1:5667 sh-c:'bash -i 2>&1'
  
*  `udp:<hostport>` - send and receive packets to specified UDP socket

    Aliases: `udp-connect:` `connect-udp:` `c-udp:` `udp-c:`
    
*  `udp-listen:<hostport>` - bind to socket on host and port

    Aliases: `udp-l:`, `l-udp:`, `listen-udp:`
    
    Note that it is not a multiconnect specifier: entire lifecycle
    of the UDP socket is the same connection.
    
    Packets get sent to the most recent seen peer.
    If no peers are seen yet, it waits for the first packet.
    
    File a feature request on Github if you want proper DNS-like request-reply UDP mode here.
  
*   `ws-connect:<spec>` - low-level WebSocket connector

    A combining specifier. Underlying specifier is should be after the colon.
    URL and Host: header being sent are independent from underlying specifier
    Aliases: `ws-c:` `c-ws:` `connect-ws:`
    
    Example: connect to echo server in more explicit way
    
        websocat --ws-c-uri=ws://echo.websocket.org/ - ws-c:tcp:174.129.224.73:80
  
*   `autoreconnect:<spec>` - Auto-reconnector

    Re-establish underlying specifier on any error or EOF
    
    Example: keep connecting to the port or spin 100% CPU trying if it is closed.
    
        websocat - autoreconnect:tcp:127.0.0.1:5445
      
    TODO: implement delays
    
*  `reuse:<spec>` - Reuse one connection for serving multiple clients

    Better suited for unidirectional connections
    
    Example (unreliable): don't disconnect SSH when websocket reconnects
      
        websocat ws-l:[::]:8088 reuse:tcp:127.0.0.1:22

*  `threadedstdio:` - Stdin/stdout, spawning a thread
  
    Like `-`, but forces threaded mode instead of async mode
    Use when standard input is not `epoll(7)`-able.
    Replaces `-` when `no_unix_stdio` Cargo feature is activated
  
*  `mirror:` - Simply copy output to input

    Similar to `exec:cat`.
  
*  `open-async:<path>` - Open file for read and write and use it like a socket

    Not for regular files, see `readfile:` and `writefile:` instead.
  
    Example:
    
        websocat - open-async:/dev/null
      
*  `open-fd:<number>` - Use specified file descriptor like a socket
  
*   `unix-connect:<path>` - Connect to UNIX socket
    Aliases: `unix:`, `connect-unix:`, `unix-c:`, `c-unix:`
    
*   `unix-listen:<path>` - Listen for connections on a UNIX socket
    Aliases: `unix-l:`, `listen-unix:`, `l-unix:`
    
    Example: with nginx
    
        umask 0000
        websocat --unlink ws-l:unix-l:/tmp/wstest tcp:[::]:22
      
    Nginx config:
    
```
    location /ws {{
        proxy_read_timeout 7d;
        proxy_send_timeout 7d;
        #proxy_pass http://localhost:3012;
        proxy_pass http://unix:/tmp/wstest;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }}
```
      
*  `unix-dgram:<path>:<path>` - Send packets to one path, receive from the other  
    
*  `abstract-connect:<string>` - Connect to Linux abstract-namespaced socket
    Aliases: `abstract-c:`, `connect-abstract:`, `c-abstract:`, `abstract:`

*  `abstract-listen:<path>` - Listen for connections on Linux abstract-namespaced socket
    Aliases: `abstract-l:`, `listen-abstract:`, `l-abstract:`
    
*  `readfile:<path>` - synchronously read files
    Blocking on operations with the file pauses the whole process
    
    Example:
    
        websocat ws-l:127.0.0.1:8000 readfile:hello.json
      
*  `writefile:<path>` - synchronously write files

    Blocking on operations with the file pauses the whole process
    Files are opened in overwrite mode.
    
    Example:
    
        websocat ws-l:127.0.0.1:8000 reuse:writefile:log.txt
        
    TODO: `appendfile:`
  
*  `clogged:` - Do nothing

    Don't read or write any bytes. Keep connections hanging.
    
*  `literal:<string>` - Output a string, discard input.

    Ignore all input, use specified string as output.
  
*  `literalreply:<string>` - Reply with this string for each input packet

    Example:

        websocat ws-l:127.0.0.1:3456 literalreply:Hello_world
  
*  `assert:<string>` - Check the input.

    Read entire input and panic the program if the input is not equal
    to the specified string.

Loopback Speed Test
---

#### socat - 1G/s

```
$ socat tcp-l:8788,reuseaddr - > /dev/null&
[1] 16042
$ pv -i 10 /dev/zero | socat - tcp:127.0.0.1:8788
20.8GiB 0:00:20 [1.07GiB/s] [    <=>                                                                                                                ]
^C
[1]+  Stopped                 socat tcp-l:8788,reuseaddr - > /dev/null
```
#### websockat (websocket mode) - 240 M/s
```
$ ./websocat_0.4_x86_64-unknown-linux-gnu -q -u l-ws:127.0.0.1:8788 - > /dev/null&
[1] 17266
$ pv -i 10 /dev/zero | ./websocat_0.4_x86_64-unknown-linux-gnu -u - ws://127.0.0.1:8788/
INFO:websocat: Connecting to ws://127.0.0.1:8788/
INFO:websocat: Validating response...
INFO:websocat: Successfully connected
 4.9GiB 0:00:20 [ 242MiB/s] [    <=>                                                                                                                ]
^C

$ fg
./websocat_0.4_x86_64-unknown-linux-gnu -q -u l-ws:127.0.0.1:8788 - > /dev/null
^C
```
#### websocat (TCP mode, without websocket) - 1.7 G/s

```
$ ./websocat_0.4_x86_64-unknown-linux-gnu -q -u l-tcp:127.0.0.1:8788 - > /dev/null&
[1] 17899
$ pv -i 10 /dev/zero | ./websocat_nossl_0.4_i686-unknown-linux-musl -u - tcp:127.0.0.1:8788
INFO:websocat: Connected to TCP 127.0.0.1:8788
33.7GiB 0:00:20 [1.71GiB/s] [    <=>                                                                                                                ]
^C

$ fg
./websocat_0.4_x86_64-unknown-linux-gnu -q -u l-tcp:127.0.0.1:8788 - > /dev/null
^C
```


P.S. Here is oneliner to remove non-blocking mode from terminal's stdin:

    perl -we 'use Fcntl qw(F_GETFL F_SETFL O_NONBLOCK); open F, "<&=", 0; my $flags = fcntl(F, F_GETFL, 0); fcntl(F, F_SETFL, $flags & !O_NONBLOCK);'


See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
* [wscat](https://github.com/websockets/wscat)
* [websocketd](https://github.com/joewalnes/websocketd)
