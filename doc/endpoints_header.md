# Endpoints

This part describes what you can put as a positional argument for Websocat.

Typically things have `<prefix>:[<value>]` format. For example, in `tcp:127.0.0.1:1234` endpoint 
`tcp:` is prefix and `127.0.0.1:1234` is value.

Some endpoints deviate from this pattern, for example, `-` for stdin/stdout and `ws://` / `wss://` URLs
to activate the client.

Endpoints are final, leaf nodes of Specifier Stacks. Note that colon-prefixed things 
like `autoreconnect:` or `broadcast:` before endpoints are not another endpoints, but Overlays.

They are described in a neghbouring chapter.

List of endpoints:
