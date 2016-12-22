# websocat
Websocket proxy, socat-style

```
$ websocat --help
websocat 0.3.0
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Exchange binary data between binary or text websocket and something.
Socat analogue with websockets.

USAGE:
    websocat_d [FLAGS] <spec1> <spec2>

FLAGS:
    -h, --help       Prints help information
    -t, --text       Send WebSocket text messages instead of binary (unstable). Affect only ws[s]:/l-ws:
    -u, --unidirectional            Only copy from spec1 to spec2.
    -U, --unidirectional-reverse    Only copy from spec2 to spec1.
    -V, --version    Prints version information

ARGS:
    <spec1>    First specifier.
    <spec2>    Second specifier.


Specifiers can be:
  ws[s]://<rest of websocket URL>   Connect to websocket
  tcp:host:port                     Connect to TCP
  unix:path                         Connect to UNIX socket
  abstract:addr                     Connect to abstract UNIX socket
  l-ws:host:port                    Listen unencrypted websocket
  l-tcp:host:port                   Listen TCP connections
  l-unix:path                       Listen for UNIX socket connections on path
  l-abstract:addr                   Listen for UNIX socket connections on abstract address
  -                                 stdin/stdout
  exec:program                      spawn a program (no arguments)
  sh-c:program                      execute a command line with 'sh -c'
  (more to be implemented)
  
Examples:
  websocat l-tcp:0.0.0.0:9559 ws://echo.websocket.org/
    Listen port 9959 on address :: and forward 
    all connections to a public loopback websocket
  websocat l-ws:127.0.0.1:7878 tcp:127.0.0.1:1194
    Listen websocket and forward connections to local tcp
    Use nginx proxy for SSL if you want
  websocat - wss://myserver/mysocket
    Connect stdin/stdout to a secure web socket.
    Like netcat, but for websocket.
    `ssh user@host -o ProxyHommand "websocat - ws://..."`
  websocat ws://localhost:1234/ tcp:localhost:1235
    Connect both to websocket and to TCP and exchange data.
  websocat l-ws:127.0.0.1:8088 sh-c:"ping 8.8.8.8 -c 1"
    Execute a command line on each connection (not for Windows)
    
Specify listening part first, unless you want websocat to serve once.

IPv6 supported, just use specs like `l-ws:::1:4567`

Web socket usage is not obligatory, you can use any specs on both sides.
If you want wss:// server, use socat or nginx in addition.
```

Pre-built binaries for Linux (usual and musl), Windows and OS-X are available on the [releases page](https://github.com/vi/websocat/releases). They are build against customized, faster [websocket library](https://github.com/cyderize/rust-websocket), but many can't connect to secure wss:// websockets, only ws://.

Limitations
---

* Slower than socat
* Can't reply to WebSocket pings

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
