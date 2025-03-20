# Glossary

* **Specifier** - WebSocket URL, TCP socket address or other connection type Websocat recognizes, 
or an overlay that transforms other Specifier.
* **Endpoint** - leaf-level specifier that directly creates some sort of Socket, without requiring another Socket first.
* **Overlay** - intermediate specifier that transforms inner specifier. From overlay's viewpoint, inner socket is called Downstream and whatever uses the overlay is called Upstream.
* **Socket** - a pair of byte stream- or datagram-oriented data flows: write (sink) and read (source), optionally with a hangup signal. Can be stream- and packet-oriented.
* **Incomplete socket** - socket where one of direction (reader or writer) is absent (null). Not to be confused with half-shutdown socket that can be read, but not written.
* **Scenario** = **Websocat Rhai Script** - detailed instruction of how Websocat would perform its operation.
Normally it is generated automatically from CLI arguments, then executed; but you can separate 
those steps and customize the scenario to fine tune of how Websocat operates. Just like usual CLI API, 
Rhai functions API is also intended to be semver-stable API of Websocat.
* **Scenario function** - Rhai native function that Websocat registers with Rhai engine that can be used 
in Scenarios.
* **Scenario Planner** - part of Websocat implementation that parses command line arguments and prepares a Scenario
* **Scenario Executor** - part of Websocat implementation that executes a Scenario.
* **CLI arguments** - combination of a positional arguments (typically Specifiers) and various flags (e.g. `--binary`) and options (e.g. `--buffer-size 4096`) that affect Scenario Planner. Sometimes, in narrow sense, it may refer to an individual block of `--compose`-ed arguments.
* **CLI API** - Things in Websocat that are accessible when starting Websocat executable and supplying various command-line arguments (except of `-x` or `--no-fixups`). This is expected to be more stable and easier to use, but less flexible compared to Scenario Functions.
* **Packet** = **Datagram** = **Message** - A byte buffer with associated flags. Correspond to one WebSocket message. Within WebSocket, packets can be split to chunks, but that should not affect user-visible properties.
* **Chunk** = **Frame** - portion of data read or written to/from stream or datagram socket in one go. Maybe a fragment of a Message or be the whole Message.
* **Task** - a logical thread of execution. Rhai code is expected to create and combine some tasks. Typically each connection runs in its own task. Corresponds to Tokio tasks.
* **Hangup** - similar to Task, but used in context of signaling various events, especially abrupt reset of sockets.
* **Specifier Stack** - Individual components of a Specifier - Endpoint and a vector of Overlays.
* **Left side**, **first specifier** - first positional argument you have specified at the left side of the Websocat CLI invocation (maybe after some transformation). Designed to handle both one-time use connectors and multi-use listeners.
* **Right side**, **second specifier** - second positional argument of the Websocat CLI invocation (may be auto-generated). Designed for single-use things to attach to connections obtained from the Left side.
* **Listener** - Type of Specifier that waits for incoming connections, spawning a task with a Socket for each incoming connection.
