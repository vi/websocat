There are more plans and ideas of what to do next in Websocat4.

Firstly, some important functions from Websocat1 should be [ported](https://github.com/vi/websocat/issues/276) to Websocat4.

Then it will mostly depend on my own needs, inspiration and ongoing Github and other issues and questions.

# For beta

* TLS server
* TLS client auth
* outgoing pings
* preamble/preamble-reverse
* More datagram tools (autoreconnect, foreachmsg, literalreply)
* base64 mode, text and binary prefixes
* one message mode
* broadcast reuser
* inhibit `Host:` header when explicit one specified / http proxy option

# Later

* Lints for unused options
* one message mode
* More datagram tools (combine/fanout, limit)
* (non-Websocket) HTTP games
* HTTP/2 games
* Proxies
* TCP reset monitoring
* Rich console with history
* Compression overlay
* base64 overlay
* async stdio
* JSON RPC tool
* exit of specific byte mode
* print ping RTTs
* timestamp overlay
* expose more TLS options to high-level UI
* just-generate-accept and just-generate-key
* max messages
* speed limit
* conncap (parallel connections limiter)
* outgoing ping limiter
* static files / strict URI mode
* inetd mode
* clogged
* socks5 proxy
* http proxy
* Prometheus integration
* simple encryptor/decryptor
* waitfordata
* left-to-right features to set envvars based on incoming request
* Good, helpful `-v` logs
* Automatic percent-encoding of URLs instead of `invalid uri character`.
* DTLS
* SCTP
* QUIC

* Play with cargo-auditable
