# Running Floresta

So far in the Floresta journey we have mostly learned about the two core library crates: `floresta-chain` and `floresta-wire`.

In [Chapter 1](ch01-00-project-overview.md) we went over the common library (`floresta-common`), the three extra libraries (`floresta-watch-only`, `floresta-compact-filters`, and `floresta-electrum`) and the `floresta` meta-crate. We also mentioned `floresta-rpc` and `floresta-node`, the libraries powering our two current binaries.

This chapter puts it all together leveraging the assembler library `floresta-node`, which provides:

- The `Florestad` struct: the "orchestrator" that starts the node, spawning asynchronous tasks for each subsystem (the `UtreexoNode`, an optional JSON-RPC server, the Electrum server, etc.). It also sends the stop signal when requested to, as the main loop in `florestad` keeps checking for a `Ctrl+C` signal.

- The `Config` type: a single configuration struct used to bootstrap `Florestad`.
