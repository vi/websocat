# websocat
Websocket proxy, socat-style

**Note: Currently SSL-enabled version may be unbuildable on systems with newer libssl because of hard reliance on old versions of dependencies. Wait for websockat 1.0.0 with async and updated deps.**

```
websocat 0.4.0
Vitaly "_Vi" Shukela <vi0oss@gmail.com>
Exchange binary data between binary or text websocket and something.
Socat analogue with websockets.

USAGE:
    websocat [FLAGS] [OPTIONS] <spec1> <spec2>

FLAGS:
    -h, --help                      Prints help information
    -q, --quiet                     No logging to stderr. Overrides RUST_LOG. Use in inetd mode.
    -t, --text                      Send WebSocket text messages instead of binary (unstable). Affects only ws[s]:/l-ws:
    -u, --unidirectional            Only copy from spec1 to spec2.
    -U, --unidirectional-reverse    Only copy from spec2 to spec1.
        --unlink                    Delete UNIX server socket file before binding it.
    -V, --version                   Prints version information

OPTIONS:
        --chmod <chmod>    Change UNIX server socket permission bits to this octal number.

ARGS:
    <spec1>    First specifier.
    <spec2>    Second specifier.


Specifiers can be:
  ws[s]://<rest of websocket URL>   Connect to websocket
  tcp:host:port                     Connect to TCP
  unix:path                         Connect to UNIX socket
  abstract:addr                     Connect to abstract UNIX socket (Linux-only)
  l-ws:host:port                    Listen unencrypted websocket
  l-ws-unix:path                    Listen unecrypted UNIX-backed websocket on addr
  l-ws-abstract:addr                Listen unecrypted abstract-UNIX-backed websocket on addr
  l-tcp:host:port                   Listen TCP connections
  l-unix:path                       Listen for UNIX socket connections on path
  l-abstract:addr                   Listen for UNIX socket connections on abstract address
  -                                 stdin/stdout
  inetd:                            stdin/stdout
  inetd-ws:                         stdin/stdout, serve one WebSocket client
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
  websocat -U l-ws:127.0.0.1:8088 sh-c:"ping 8.8.8.8 -c 1"
    Execute a command line on each connection (not for Windows)
  ssh -c ProxyCommand="websocat - ws://myserver/mywebsocket" user@myserver
    Use SSH connection wrapped in a web socket
  websocat l-ws:0.0.0.0:80 tcp:127.0.0.1:22
    Server part of the command above
  websocat l-ws-unix:/tmp/sshws.sock tcp:127.0.0.1:22
    Like previous example, but for integration with NginX using UNIX sockets
    Nginx config snippet example:
    location /mywebsocket {
        proxy_read_timeout 1h;
        proxy_send_timeout 1h;
        #proxy_pass http://localhost:3012;
        proxy_pass http://unix:/tmp/sshws.sock;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
    Don't forget about --chmod and/or --unlink
  inetd config line:
    1234 stream tcp nowait myuser  /path/to/websocat websocat --quiet inetd-ws: tcp:127.0.0.1:22

    
Specify listening part first, unless you want websocat to serve once.

IPv6 supported, just use specs like `l-ws:::1:4567`

Web socket usage is not obligatory, you can use any specs on both sides.
If you want wss:// server, use socat or nginx in addition.
```

Pre-built binaries for Linux (usual and musl), Windows, OS X and Android (ARM) are available on the [releases page](https://github.com/vi/websocat/releases). Most are built without SSL support, so can't connect to secure wss:// websockets, only ws://.

Limitations
---

* Speed overhead compared to plain TCP
* Can't reply to WebSocket pings
* EOF and half-shutdown socket handling may be subpar.
* No UDP
* exec: can't accept array of arguments (TODO)
* SSL support may be unstable due to [reliance on a function that is now deprecated and removed](https://github.com/cyderize/rust-websocket/issues/125).

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

See also
---

* [wstunnel](https://github.com/erebe/wstunnel)
* [wscat](https://github.com/websockets/wscat)
* [websocketd](https://github.com/joewalnes/websocketd)
