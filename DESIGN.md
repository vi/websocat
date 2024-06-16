# Design principles of Websocat4

* Starting up Websocat and connection establishment is allowed to be slow (i.e. allowed to parse strings, allocate memory, walk trees). Data transfer in already established connections should be fast.
* **Rhai** is used as a "control plane" and decides what connect to what and how transform layers are connected to each other.
* When serving multiple connections, it is OK to execute Rhai code each time. It is not OK to execute Rhai code for each transferred packet within a connection.
* Rhai script fully defines Websocat behaviour. Most other CLI options just affect how Rhai script is generated. Filenames / process arguments get serialized as bytes, maybe inserted as base64 into the script if needded.
* It is OK to have Rhai functions that are reachable only when explicitly specifying the script (unused by high-level CLI).
* Each connection (or connection direction) is either bytestream-oriented or datagram-oriented. Bytestream-oriented use `tokio::io::{AsyncRead, AsyncWrite}`, datagram-oriented connections use custom traits (not `Stream<Bytes>` / `Sink<Bytes>`).
* Custom traits for datagram connections are geared towards Websocket use case and allows typical use cases without allocations or excessive copying data around each invocation. Unusual decisions include mutable buffer for writer (to aid in-place masking for WebSocket) and partial buffer immutability gurantees for readers.
* WebSocket framing library is custom, designed with avoiding allocations in mind.
* It is OK to have multiple indirections for each transferred data chunk (i.e. `dyn` layer between stdio and line wrapper, then between line wrapper and WebSocket framer, then between the framer and TLS layer, then between the TLS layer and TCP socket).
* `fn poll` is also the way, not everything needs to be async.
* It is OK to add more elaborate `scenario_executor` things to simplify Websocat's Rhai scripts.
* Address types and overlay stacks use `enum` approach, not `dyn`.

# History or Websocat redesigns

Fortunately, Websocat v1 remained maintained all the way though this experimentation.

## 0.4

Sync version, used thread per data transfer direction.
Used `websocket` crate, `clap` for CLI.

## 1.0

Based on tokio-core, later on Tokio v0.1.
Specifier stacks, using `dyn`-based pluggable overlays and address types.
Locked to single async IO thread only. Uses `Rc<RefCell>` a lot.
Iffy error handing.
Still uses `websocket` crate.

## 2.0

Attempt to upgrade to Tokio 0.3 (and later Tokio 1.0) and Hyper 0.14 and make it multi-crate.
Attempt to migrate `websocket` crate from hyper 0.10 to hyper 0.14.

Abandoned attempts to make arch saner incrementally.

## 3.0

"[Second-system](https://en.wikipedia.org/wiki/Second-system_effect)" edition.

Based on Tokio 1.0 from the beginning, based on `tokion-tungstenite`.
Custom CLI framework, custom lisp-esque low-level specifier language (there is even a quasi-quote operator inside).
Playground for rolling my own proc macros.

Even reached an alpha Github release, but I after thinking about flexibility
and complexity and memory allocations I decided to count it as educational efforts and move on.