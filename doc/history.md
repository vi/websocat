# History or Websocat redesigns

Websocat started in 2016. In 2018 I decided that it's time to call it 1.0.0.

Eventually it has became my most popular project, so I keep extending and maintaining it.

However, v1.0.0 is based on legacy dependencies and does not use async/await (and has some other inconvenient design decisions), so I started redesigning it. It took multiple attempts to find the way I am satisfied with, but even abandoned attempts provided good educational value.

Fortunately, Websocat v1 remained maintained all the way through this experimentation.

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

Based on Tokio 1.0 from the beginning, based on `tokio-tungstenite`.
Custom CLI framework, custom lisp-esque low-level specifier language (there is even a quasi-quote operator inside).
Playground for rolling my own proc macros.

Even reached an alpha Github release, but I after thinking about flexibility
and complexity and memory allocations I decided to count it as educational efforts and move on.
