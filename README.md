# websocat
Websocket proxy, socat-style

```
USAGE:
    websocat <spec1> <spec2>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <spec1>    First specifier.
    <spec2>    Second specifier.


Specifiers can be:
  ws[s]://<rest of websocket URL>   Connect to websocket
  l-ws:host:port                    Listen unencrypted websocket
  tcp:host:port                     Connect to TCP
  l-tcp:host:port                   Listen TCP connections
  -                                 stdin/stdout
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
  websocat ws://localhost:1234/ tcp:localhost:1235
    Connect both to websocket and to TCP and exchange data.
    
Specify listening part first, unless you want websocat to serve once.

IPv6 supported, just use specs like `l-ws:::1:4567`

Web socket usage is not obligatory, you any specs on both sides.
```

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
