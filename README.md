# websocat
Websocket proxy, socat-style

```
USAGE:
    websocat <listener_spec> <connector_spec>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <listener_spec>     Listener specifier.
    <connector_spec>    Connector specifier.


Specifiers are:
  ws[s]://<rest of websocket URL>    websockets
  -                                  stdin/stdout
  (more to be implemented)
  
Examples:
  websocat - wss://myserver/mysocket
    Connect stdin/stdout to secure web socket once.
    Currently it is the only working example.
```

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
