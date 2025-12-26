#!/bin/bash

if [ ! -f misc/scan_source.py ]; then
   echo "Wrong current directory"
   exti 1
fi

set -e
python3 misc/scan_source.py > outline.json
set +e
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

Use https://websocat.net/websocat4/ (or 'doc' directory in the source code) for reference of all Websocat functions
EOF

crcargo build # to update the --help message

cat > doc/endpoints.md <<'EOF'
<!-- Note: this file is auto-generated -->
{{#include endpoints_header.md}}
EOF

TODOC=endpoints python misc/doc_specifiers.py < outline.json >> doc/endpoints.md

cat > doc/overlays.md <<'EOF'
<!-- Note: this file is auto-generated -->
{{#include overlays_header.md}}
EOF

TODOC=overlays python misc/doc_specifiers.py < outline.json >> doc/overlays.md

cat > doc/functions.md <<'EOF'
<!-- Note: this file is auto-generated -->
{{#include functions_header.md}}
EOF

python misc/doc_functions.py < outline.json >> doc/functions.md

cat > doc/clihelp.md <<'EOF'
<!-- Note: this file is auto-generated -->
## `--help` output

```
EOF
./target/mydev/websocat --help >> doc/clihelp.md

echo '```' >> doc/clihelp.md
