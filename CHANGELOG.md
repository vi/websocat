<a name="unreleased"></a>
# Unreleased

* `waitfordata:` overlay to delay connection initiation until first data is attempted to be written to it
* Dockerfile updates
* `lengthprefixed:` overlay - alternative to base64 mode

<a name="v1.12.0"></a>
# [Maintainance release (v1.12.0)](https://github.com/vi/websocat/releases/tag/v1.12.0) - 17 Sep 2023

* Option to stop sending or replying to WebSocket pings after specified amount of sent or received pings (for testing idling disconnection behaviour of counterparts).
* `--exec-exit-on-disconnect`
* Print `Location:` header value in error message when facing a redirect instead of a WebSocket connection.
* Other minor fixes

[Changes][v1.12.0]


<a name="v1.11.0"></a>
# [Still keeping v1 afloat instead of concentrating on v3 (v1.11.0)](https://github.com/vi/websocat/releases/tag/v1.11.0) - 24 Sep 2022

* `--preamble` (`-p`) options to prepend static text to Websocat sessions. For use to authenticate and subscribe to something over WebSocket. Note that specifying passwords on command line may be insecure. Also command line handling around `-p` is finicky. There is also `--preamble-reverse` (`-P`) option to prepend similar chunk in the reverse direction.
* `--compress-{zlib,deflate,gzip}` and respective `--uncompress-...` options to modify binary WebSocket messages going to/from a WebSocket. Note that it is not related to [permessage-deflate](https://www.rfc-editor.org/rfc/rfc7692.html), which does similar thing, but on lower level.
* `exit_on_specific_byte:` overlay to trigger exit when specific byte is encountered. For interactive tty usage.
* `--client-pkcs12-der` to specify client identity certificate for connecting to `wss://` or `ssl:` that requires mutual authentication.
* `openssl-probe` is now active by default on Linux, to support for overriding CA lists using environment variables.
* Incoming WebSocket frames and message are now limited by default, to prevent memory stuffing denial of service. But the default limit is big (100 megabytes). Use `--max-ws-frame-length` and `--max-ws-message-length` options to override.
* `Cargo.lock` is now oriented for building with modern Rust compiler. There is `Cargo.lock.legacy` with dependencies manually locked to versions that support Rust 1.46.0.

[Changes][v1.11.0]

<a name="v1.10.0"></a>
# [Some fixes, some features. (v1.10.0)](https://github.com/vi/websocat/releases/tag/v1.10.0) - 17 May 2022

* Add `--close-status-code` and ` --close-reason`
* Fix `--queue-len` option that took no effect
* Fix racing to connect to multiple resolved addresses in `tcp:` specifier (i.e. "happy eyeballs") - now it skips errors if there is a working connection. This does not fix `ws://localhost` unfortunately.
* `crypto:` overlay and associated options
* `prometheus:` overlay and associated options
* `random:` specifier

[Changes][v1.10.0]

<a name="v1.9.0"></a>
# [Supposedly without yanked crates (v1.9.0)](https://github.com/vi/websocat/releases/tag/v1.9.0) - 30 Oct 2021

* `ssl` Cargo feature is now enabled by default
* `vendored_openssl` Cargo feature is now not enabled by default
* `--stdout-announce-listening-ports` option to print message when server port is ready to accept clients.
* `--no-close` option now also affects Websocket server mode, not just client
* `timestamp:` overlay to mangle message, prepending current timestamp as text
* `--print-ping-rtts` option
* Updated deps for [#138](https://github.com/vi/websocat/issues/138) (not checked whether all yanks are resolved although).

[Changes][v1.9.0]

<a name="v1.8.0"></a>
# [Fix some bugs (v1.8.0)](https://github.com/vi/websocat/releases/tag/v1.8.0) - 15 Apr 2021

* `--accept-from-fd` option for better systemd intergration
* `exec:`/`cmd:`/`sh-c:` specifiers now don't terminate process prematurely
* `--foreachmsg-wait-read` for better `foreachmsg:` overlay behaviour. Now `foreachmsg:exec:./myscript` is more meaningul.
* ` --basic-auth` option to insert basic authentication header more easily
* Websocket close message is now logged in debug mode

[Changes][v1.8.0]

<a name="v1.7.0"></a>
# [Default threaded stdio, `log:` filter (v1.7.0)](https://github.com/vi/websocat/releases/tag/v1.7.0) - 22 Feb 2021

* Websocat now does not set terminal to nonblocking mode if isatty by default. This should help with [#76](https://github.com/vi/websocat/issues/76).
* New overlay `log:` that prints bytes as they travel though Websocat, for debugging.

[Changes][v1.7.0]

<a name="v1.6.0"></a>
# [A heartbeat release (v1.6.0)](https://github.com/vi/websocat/releases/tag/v1.6.0) - 08 Jul 2020

* UDP multicast options
* `foreachmsg:` overlay - run specifier (i.e. connect somewhere or execute a program) on each WebSocket message instead of on each WebSocket connection.
* Various minor options like `--max-messages` or zero-length message handling.
* Low-level Websocket features: `--just-generate-key` and `--just-generate-accept` options which help generating HTTP headers for WebSockets. `ws-lowlevel-server:` and `ws-lowlevel-client:` overlays to use expose WebSocket's data encoder/decoder without HTTP part.
* Basic `http://` client with arbitrary method, uri and so on.
* Delay for `autoreconnect:` overlay
* More pre-built release assets
* Base64 mode for binary WebSocket messages
* Prefixes for text and binary WebSocket messages, allowing to discriminate incoming binary and text WebSocket messages and intermix outgoing binary and text WebSocket messages.
* Sort-of-unfinished `http-post-sse:` specifier allowing to use HTTP server-sent events (in one direction) and POST request bodies (in the other direction) instead of (or in addition to) a WebSocket and to bridge them together. This mode is not tested properly although.

[Changes][v1.6.0]

<a name="v1.5.0"></a>
# [Client basic auth, header-to-env (v1.5.0)](https://github.com/vi/websocat/releases/tag/v1.5.0) - 18 Aug 2019

* Using client URI's like `websocat ws://user:password@host/` now adds basic authentication HTTP header to request
* New command-line option: `--header-to-env`
* Minor dependencies update
* Built with newer Rust on newer Debian

[Changes][v1.5.0]

<a name="v1.4.0"></a>
# [WebSocket ping and Sec-WebSocket-Protocol improvements (v1.4.0)](https://github.com/vi/websocat/releases/tag/v1.4.0) - 21 Mar 2019

* New options: `--server-protocol`, `--ping-timeout`, `--ping-interval`, `--server-header`
* Fixed replying to WebSocket pings
* Fixed replying to requests with `Sec-WebSocket-Protocol`.

[Changes][v1.4.0]

<a name="v1.3.0"></a>
# [tokio, conncap, pkcs12-passwd, typos (v1.3.0)](https://github.com/vi/websocat/releases/tag/v1.3.0) - 06 Mar 2019

[Changes][v1.3.0]

<a name="v1.2.0"></a>
# [-k (--insecure), native-tls (v1.2.0)](https://github.com/vi/websocat/releases/tag/v1.2.0) - 01 Nov 2018

[Changes][v1.2.0]

<a name="v1.1.0"></a>
# [More features (v1.1.0)](https://github.com/vi/websocat/releases/tag/v1.1.0) - 30 Aug 2018

* Static files aside from the websocket for easy prototyping
* SOCKS5 proxy client
* wss:// listener
* Setting environment variables for `exec:`
* Sending SIGHUP signal to child process on client disconnect
* `--jsonrpc` mode

[Changes][v1.1.0]

<a name="v1.1-pre"></a>
# [Preview of 1.1 (v1.1-pre)](https://github.com/vi/websocat/releases/tag/v1.1-pre) - 13 Jul 2018

* --set-environment option and --static-file

[Changes][v1.1-pre]


<a name="v1.0.0"></a>
# [The release. Finally. (v1.0.0)](https://github.com/vi/websocat/releases/tag/v1.0.0) - 04 Jul 2018

[Changes][v1.0.0]


<a name="v1.0.0-beta"></a>
# [Refactor and more features (v1.0.0-beta)](https://github.com/vi/websocat/releases/tag/v1.0.0-beta) - 20 Jun 2018

[Changes][v1.0.0-beta]

<a name="v1.0.0-alpha"></a>
# [Async alpha (v1.0.0-alpha)](https://github.com/vi/websocat/releases/tag/v1.0.0-alpha) - 10 May 2018

[Changes][v1.0.0-alpha]

<a name="v0.5.1-alpha"></a>
# [Async preview (v0.5.1-alpha)](https://github.com/vi/websocat/releases/tag/v0.5.1-alpha) - 14 Mar 2018

[Changes][v0.5.1-alpha]

<a name="v0.4.0"></a>
# [Forked rust-websocket (v0.4.0)](https://github.com/vi/websocat/releases/tag/v0.4.0) - 18 Jan 2017

[Changes][v0.4.0]

<a name="v0.3.0"></a>
# [More features (v0.3.0)](https://github.com/vi/websocat/releases/tag/v0.3.0) - 22 Dec 2016

- Unix sockets
- Executing programs and command lines
- Unidirectional mode
- Text mode (don't rely on it)

[Changes][v0.3.0]

<a name="v0.2.0"></a>
# [First actual release (v0.2.0)](https://github.com/vi/websocat/releases/tag/v0.2.0) - 24 Nov 2016

[Changes][v0.2.0]


[v1.12.0]: https://github.com/vi/websocat/compare/v1.11.0...v1.12.0
[v1.11.0]: https://github.com/vi/websocat/compare/v1.10.0...v1.11.0
[v1.10.0]: https://github.com/vi/websocat/compare/v3.0.0-prealpha0...v1.10.0
[v1.9.0]: https://github.com/vi/websocat/compare/v1.8.0...v1.9.0
[v1.8.0]: https://github.com/vi/websocat/compare/v1.7.0...v1.8.0
[v1.7.0]: https://github.com/vi/websocat/compare/v1.6.0...v1.7.0
[v1.6.0]: https://github.com/vi/websocat/compare/v2.0.0-alpha0...v1.6.0
[v1.5.0]: https://github.com/vi/websocat/compare/v1.4.0...v1.5.0
[v1.4.0]: https://github.com/vi/websocat/compare/v1.3.0...v1.4.0
[v1.3.0]: https://github.com/vi/websocat/compare/v1.2.0...v1.3.0
[v1.2.0]: https://github.com/vi/websocat/compare/v1.1.0...v1.2.0
[v1.1.0]: https://github.com/vi/websocat/compare/v1.1-pre...v1.1.0
[v1.1-pre]: https://github.com/vi/websocat/compare/v1.0.0...v1.1-pre
[v1.0.0]: https://github.com/vi/websocat/compare/v1.0.0-beta...v1.0.0
[v1.0.0-beta]: https://github.com/vi/websocat/compare/v1.0.0-alpha...v1.0.0-beta
[v1.0.0-alpha]: https://github.com/vi/websocat/compare/v0.5.1-alpha...v1.0.0-alpha
[v0.5.1-alpha]: https://github.com/vi/websocat/compare/v0.4.0...v0.5.1-alpha
[v0.4.0]: https://github.com/vi/websocat/compare/v0.3.0...v0.4.0
[v0.3.0]: https://github.com/vi/websocat/compare/v0.2.0...v0.3.0
[v0.2.0]: https://github.com/vi/websocat/tree/v0.2.0

<!-- Generated by https://github.com/rhysd/changelog-from-release v3.7.1 -->
