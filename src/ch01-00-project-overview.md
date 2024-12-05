# Project Overview

The Floresta project is made up of a few library crates, providing modular node components, and the `florestad` binary (i.e. Floresta daemon), assembling components for a full node.

Developers can use the core components in the libraries to build their own wallets and node implementations. They can use individual libraries or use the whole pack of components with the `floresta` meta-crate, which just re-exports libraries.

The libraries in Floresta are:
- `floresta-chain`
  - Validates the blockchain and tracks the state.
- `floresta-cli`
  - Provides command-line interface tools to interact with `florestad`.
- `floresta-common`
  - Contains shared data structures and traits used across other crates.
- `floresta-compact-filters`
  - Implements compact filter functionality for wallets.
- `floresta-electrum`
  - An Electrum server implementation.
- `floresta-watch_only`
  - A watch-only wallet implementation, optimized for Electrum servers.
- `floresta-wire`
  - Handles network communication and data fetching.
- `floresta`
  - A meta-crate that re-exports these components.

The most important libraries are `floresta-chain`, to validate the chain and keep track of the state, and `floresta-wire`, to fetch network data. We need both kinds of components to construct a full node.

### A Car Analogy

A full node is like a self-driving car that must keep up with a constantly moving destination, as new blocks are produced. The **Bitcoin network** is the road. `Floresta-wire` acts as the car's sensors and navigation system, gathering data from the road (transactions, blocks) and feeding it to `Floresta-chain`, which is the engine and control system that moves the car forward. Without `Floresta-wire`, the engine and control system wouldn't know what to do, and without `Floresta-chain`, the car wouldn't move at all.

Both components must work properly: if `Floresta-wire` fails or provides incorrect data, the car will either be paralyzed, unable to reach the destination (blockchain sync), or misled, arriving at the wrong destination (syncing to an incorrect chain). If `Floresta-chain` fails, the car might get stuck or follow the wrong path because the control system isn't working properly, even with correct directions from `Floresta-wire`.
