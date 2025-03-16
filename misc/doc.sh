#!/bin/sh

if [ ! -f misc/scan_source.py ]; then
   echo "Wrong current directory"
   exti 1
fi

python3 misc/scan_source.py > outline.json
python3 misc/list_planner_content.py  > src/help_addendum.txt  < outline.json 

cat >> src/help_addendum.txt <<EOF

Examples:

  websocat ws://127.0.0.1:1234
    Simple WebSocket client

  websocat -s 1234
    Simple WebSocket server

  websocat -b tcp-l:127.0.0.1:1234 wss://ws.vi-server.org/mirror
    TCP-to-WebSocket converter

  websocat -b ws-l:127.0.0.1:8080 udp:127.0.0.1:1234
    WebSocket-to-UDP converter

Use doc.md for reference of all Websocat functions
EOF

crcargo build # to update the --help message
python3 misc/doc.py  > doc.md <  outline.json
