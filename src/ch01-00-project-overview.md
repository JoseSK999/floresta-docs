# Project Overview

The Floresta project is made up of a few library crates, providing modular node and application components, and two binaries: `florestad`, the Floresta daemon (i.e., the assembled node implementation), and `floresta-cli`, the command-line interface for `florestad`.

Developers can use the core components in the libraries to build their own wallets and node implementations. They can use individual libraries or use the whole pack of components with the `floresta` meta-crate, which just re-exports libraries.

## Components of Floresta

At the heart of Floresta are two fundamental libraries:

- `floresta-chain`, which validates the blockchain and maintains node state.
- `floresta-wire`, which connects to the Bitcoin network, fetching transactions and blocks to advance the chain.

> A useful way to picture the role of these two libraries is with a car analogy:
> 
> A full node is like a self-driving car that must keep up with a constantly moving destination, as new blocks are produced. The **Bitcoin network** is the road. `floresta-wire` is the car's sensors and navigation, reading the road ahead. `floresta-chain` is the engine and control system, deciding how to move forward. Without `floresta-wire`, the engine has no data to act on; without `floresta-chain`, the car cannot move.

These two crates share building blocks from `floresta-common` (a small common library). Together, they form the minimum needed for a functioning node. On top of them, utilities extend functionality:

- `floresta-watch-only`: a watch-only wallet backend for tracking addresses and balances.
- `floresta-compact-filters`: builds and queries BIP-158 filters to speed wallet rescans and enable UTXO lookups. This is especially useful as Floresta is fully pruned.
- `floresta-electrum`: an Electrum server that answers wallet-oriented queries (headers, balances, UTXOs, transactions) for external clients.

Finally, we find a meta-crate that re-exports these components, and two libraries that power the `floresta-cli` and `florestad` binaries:

- `floresta`
  - A meta-crate that re-exports the previous modular components.
- `floresta-rpc`
  - Provides the JSON-RPC API and types used by the CLI, powering `floresta-cli`.
- `floresta-node`
  - Implements the node functionality by assembling all the components. This crate is the "glue layer" between the modular crates and the daemon binary (`florestad` will just invoke the logic implemented here).
