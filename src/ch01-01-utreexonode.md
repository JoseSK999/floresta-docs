## UtreexoNode

`UtreexoNode` is the top-level type in Floresta, responsible for managing P2P connections, receiving network data, and broadcasting transactions. All its logic is found at the `floresta-wire` crate.

Blocks fetched by `UtreexoNode` are passed to a blockchain backend for validation and state tracking. This backend is represented by a generic `Chain` type. Additionally, `UtreexoNode` relies on a separate generic `Context` type to provide context-specific behavior.

![](./img/project-org.png)

*Figure 1: Diagram of the UtreexoNode type.*

Below is the actual type definition, which is a struct with two fields and trait bounds for the `Chain` backend.

Filename: floresta-wire/src/p2p_wire/node.rs

```rust
# // Path: floresta-wire/src/p2p_wire/node.rs
#
pub struct UtreexoNode<Chain: BlockchainInterface + UpdatableChainstate, Context> {
    pub(crate) common: NodeCommon<Chain>,
    pub(crate) context: Context,
}
```

The `Chain` backend must implement two key traits:

- `UpdatableChainstate`: Provides methods to update the state of the blockchain backend.
- `BlockchainInterface`: Defines the interface for interacting with other components, such as the `UtreexoNode`.

Both traits are located in the `floresta-chain` library crate, in _src/pruned_utreexo/mod.rs_.

In the next section, we will explore the required API for these traits.

{{#quiz ../quizzes/ch01-01-utreexonode.toml}}
