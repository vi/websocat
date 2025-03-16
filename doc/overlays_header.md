# Overlays

Overlays modify some aspect of underlying connection, allowing to attach processing steps to the connection.
You can specify multiple overlays by prepending its prefix (thaat typically ends in a colon) to the specifier.

For example, if `tcp:127.0.0.1:443` is a plain TCP connection, `tls:tcp:127.0.0.1:443` is a TLS client that 
uses a TCP connection under the hood and `ws-c:tls:tcp:127.0.0.1:443` is a WebSocket client that uses a TLS 
client that uses a TCP connection.

Unlike Endpoints, Overlays do not have values (only downstream specifiers). Configuration for overlays goes to
CLI options.

There are two socket types: Bytestream and Datagrams. Overlays typically handle only one of the types, a mismatch
can lead to a failure in resuting Scenario.

Here is list of all overlays:
