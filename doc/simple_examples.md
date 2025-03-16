# Basic examples

## CLI WebSocket client

```
$ websocat wss://ws.vi-server.org/mirror
1234
1234
555555
555555
```

By default, if you just specify a WebSocket URL as a sole parameter, 
Websocat will connect to it and turn each line you types in console into 
a WebSocket text message and each incoming WebSocket message into a line.

Embedded newslines in WebSocket messages would be substituted by spaces 
to preserve one line = one message properly.

There are a number of command line switches and options to adjust details
of this behaviour, e.g. you can use `--separator-n=2` to make empty lines
act as a delimiter instead of line feeds.

## CLI WebSocket server

```
$ websocat -s 1234
```

It would start listening `ws://127.0.0.1:1234` and dumping all incoming
WebSocket messages to console. Typed lines will be also converted into
WebSocket messages aimed at clients.

It supports multiple simultaneously connected clients, though by default
only one of connected clients would get replies from console.

