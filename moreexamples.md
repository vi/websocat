# More examples: proxy servers

## Connect to a WebSocket using a SOCKS5 proxy

There is internal SOCKS5 client now, but sometimes external client is better:

    websocat -v -t - --ws-c-uri=ws://echo.websocket.org ws-c:cmd:'SOCKS5_PASSWORD=a connect-proxy -S a@127.0.0.1:9050 echo.websocket.org 80'

## Connect to a WebSocket using HTTP proxy

    websocat -v -t - --ws-c-uri=ws://echo.websocket.org ws-c:cmd:'connect-proxy -H 127.0.0.1:9051 echo.websocket.org 80'


## Listen WebSocket on SOCKS5 server side and connect to it

```
cat > port_obtained << \EOF
#!/bin/sh
echo Remote port opened: $1
websocat -t -1 literal:"Roundtrip using SOCKS server" ws://132.148.129.183:$1/
EOF

chmod +x port_obtained

websocat -E -t ws-u:socks5-bind:tcp:132.148.129.183:14124 - --socks5-destination 255.255.255.255:65535 --socks5-bind-script ./port_obtained
Remote port opened: 53467
Roundtrip using SOCKS server

websocat -v -E -t ws-u:socks5-bind:tcp:132.148.129.183:14124 - --socks5-destination 255.255.255.255:65535 --socks5-bind-script ./port_obtained 
 INFO 2018-08-29T22:04:42Z: websocat::lints: Auto-inserting the line mode
 INFO 2018-08-29T22:04:42Z: websocat::sessionserve: Serving Message2Line(WsServer(SocksBind(TcpConnect(V4(132.148.129.183:14124))))) to Line2Message(Stdio) with Options { websocket_text_mode: true, websocket_protocol: None, udp_oneshot_mode: false, unidirectional: false, unidirectional_reverse: false, exit_on_eof: true, oneshot: false, unlink_unix_socket: false, exec_args: [], ws_c_uri: "ws://0.0.0.0/", linemode_strip_newlines: false, linemode_strict: false, origin: None, custom_headers: [], websocket_version: None, websocket_dont_close: false, one_message: false, no_auto_linemode: false, buffer_size: 65536, broadcast_queue_len: 16, read_debt_handling: Warn, linemode_zero_terminated: false, restrict_uri: None, serve_static_files: [], exec_set_env: false, reuser_send_zero_msg_on_disconnect: false, process_zero_sighup: false, process_exit_sighup: false, socks_destination: Some(SocksSocketAddr { host: Ip(V4(255.255.255.255)), port: 65535 }), auto_socks5: None, socks5_bind_script: Some("./port_obtained") }
 INFO 2018-08-29T22:04:43Z: websocat::net_peer: Connected to TCP
 INFO 2018-08-29T22:04:46Z: websocat::proxy_peer: SOCKS5 connect/bind: SocksSocketAddr { host: Ip(V4(0.0.0.0)), port: 34020 }
Remote port opened: 34020
 INFO 2018-08-29T22:04:46Z: websocat::proxy_peer: SOCKS5 remote connected: SocksSocketAddr { host: Ip(V4(104.131.203.210)), port: 58836 }
 INFO 2018-08-29T22:04:47Z: websocat::ws_server_peer: Incoming connection to websocket: /
 INFO 2018-08-29T22:04:47Z: websocat::ws_server_peer: Upgraded
 INFO 2018-08-29T22:04:47Z: websocat::stdio_peer: get_stdio_peer (async)
 INFO 2018-08-29T22:04:47Z: websocat::stdio_peer: Setting stdin to nonblocking mode
 INFO 2018-08-29T22:04:47Z: websocat::stdio_peer: Installing signal handler
Roundtrip using SOCKS server
 INFO 2018-08-29T22:04:47Z: websocat::sessionserve: Forward finished
 INFO 2018-08-29T22:04:47Z: websocat::sessionserve: Forward shutdown finished
 INFO 2018-08-29T22:04:47Z: websocat::sessionserve: One of directions finished
 INFO 2018-08-29T22:04:47Z: websocat::stdio_peer: Restoring blocking status for stdin
 INFO 2018-08-29T22:04:47Z: websocat::stdio_peer: Restoring blocking status for stdin
```
