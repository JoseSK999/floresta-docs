## BestChain and DiskBlockHeader

As previously mentioned, `BestChain` and `DiskBlockHeader` are Floresta types used for storing and retrieving data in the `ChainStore` database.

### DiskBlockHeader

We use a custom `DiskBlockHeader` instead of the direct `bitcoin::block::Header` to add some metadata:

Filename: pruned_utreexo/chainstore.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/chainstore.rs
#
// BlockHeader is an alias for bitcoin::block::Header

pub enum DiskBlockHeader {
    FullyValid(BlockHeader, u32),
    AssumedValid(BlockHeader, u32),
    Orphan(BlockHeader),
    HeadersOnly(BlockHeader, u32),
    InFork(BlockHeader, u32),
    InvalidChain(BlockHeader),
}
```

`DiskBlockHeader` not only holds a header but also encodes possible block states, as well as the height when it makes sense.

When we start downloading headers in IBD we save them as `HeadersOnly`. If a header doesn't have a parent, it's saved as `Orphan`. If it's not in the best chain, `InFork`. And when we validate the actual blocks we should be able to mark the headers as `FullyValid`.

Also, we have `AssumeValid` for a configuration that allows the node to skip script validation, and `InvalidChain` for cases when `UpdatableChainstate::invalidate_block` is called.

### BestChain

The `BestChain` struct is an internal representation of the chain we are in and has the following fields:

- `best_block`: The current best chain's last `BlockHash` (the actual block may or may not have been validated yet).
- `depth`: The number of blocks pilled after the genesis block (i.e., the height of the tip).
- `validation_index`: The `BlockHash` up to which we have validated the chain.
- `alternative_tips`: A vector of fork tip `BlockHash`es with a chance of becoming the best chain.
- `assume_valid_index`: Height occupied by the assume valid block (up to which we don't validate scripts).

Filename: pruned_utreexo/chain_state.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub struct BestChain {
    pub best_block: BlockHash,
    pub depth: u32,
    pub validation_index: BlockHash,
    pub alternative_tips: Vec<BlockHash>,
    pub assume_valid_index: u32,
}
```

{{#quiz ../quizzes/ch02-02-bestchain-and-diskblockheader.toml}}
