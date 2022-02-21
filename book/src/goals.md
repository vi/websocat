# Goals of Websocat

1. To connect sockets and web worlds together. Be like [`socat` tool](http://www.dest-unreach.org/socat/), but with web features like Websockets.
2. Be a simple to use command-line WebSocket client and server for developments purposes. Be a middling difficulty to use for some tricker cases.
3. Be flexible enough both to replace many usages of `socat` and also to be used in conjunction with `socat`.
4. To enable Websockets to be used as a general-purpose tunneling mechanism e.g. for SSH or VPN.
5. To have reasonable performance for transferring data in already established connections.
6. To have reasonable reliability, enabling some not-super-important usages in production or "protoduction".
7. Be able to scan user input and provide warnings and notices ("lints") if something is likely to be wrong.

# Non-goals

* Keeping it simple internally. Websocat3 may be a case of "second system effect", there is even LISP-esque DLS inside now. There is a custom proc macro already.
* Maximizing performance. Transferring TCP data though Websocat without transforming it in any way should be not more than twice as slow as transferring it though `socat`. It is allowed to allocate memory for each handled portion of data. Connection establishement may involve some flexible logic (but should use things prepared in advance). Startup is allowed to be tricky, parsing and re-parsing text data, checking things  Think of it like using dynamic typing at startup, switching to using static types when working.

# Some technical goals

* Be able to serve as a **correct** TCP forwarder/proxy, with proper backpressure and HUP/RST handling. Currently impeded by the lack of underlying libs support, which is impeded by needing to also support Windows.
* Be able to use many socket types, including AF_UNIX, SEQPACKET and abstract-namespaced ones on Linux.
* To have architecure to support interconnecting all imagineable things in some way: SCTP, QUIC, WebRTC DataChannel, ICMP tunnel, etc. Exposing all features of those components is not a goal, just using them as a socket-like stream or datagram pipe.
* To be modular, avoiding having a central place where things need to be registered. Exceptions: list of all classes (populated by a script), short CLI options (but not long).
