# More examples

## Connect to a WebSocket using a SOCKS5 proxy

    websocat -v -t - --ws-c-uri=ws://echo.websocket.org ws-c:cmd:'SOCKS5_PASSWORD=a connect-proxy -S a@127.0.0.1:9050 echo.websocket.org 80'

## Connect to a WebSocket using HTTP proxy

    websocat -v -t - --ws-c-uri=ws://echo.websocket.org ws-c:cmd:'connect-proxy -H 127.0.0.1:9051 echo.websocket.org 80'

