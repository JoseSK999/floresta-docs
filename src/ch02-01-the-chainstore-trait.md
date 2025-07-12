## The ChainStore Trait

> `ChainStore` is a trait that abstracts the persistent storage layer for the Floresta `ChainState` backend.

To create a `ChainState`, we start by building its `ChainStore` implementation.

### ChainStore API

The methods required by `ChainStore`, designed for interaction with persistent storage, are:
- `save_roots_for_block` / `load_roots_for_block`: Save or load the utreexo accumulator (merkle roots) that results after processing a particular block.
- `save_height` / `load_height`: Save or load the current chain tip data.
- `save_header` / `get_header`: Save or retrieve a block header by its `BlockHash`.
- `get_block_hash` / `update_block_index`: Retrieve or associate a `BlockHash` with a chain height.
- `flush`: Immediately persist saved data still in memory. This ensures data recovery in case of a crash.
- `check_integrity`: Performs a database integrity check. This can be a no-op if our implementation leverages a database crate that ensures integrity.

In other words, the implementation of these methods should allow us to save and load:

- The current accumulator (serialized as a `Vec<u8>`).
- The current chain tip data (as `BestChain`).
- Block headers (as `DiskBlockHeader`), associated to the block hash.
- Block hashes (as `BlockHash`), associated with a height.

`BestChain` and `DiskBlockHeader` are important Floresta types that we will see in a minute. `DiskBlockHeader` represents stored block headers, while `BestChain` tracks the chain tip metadata.

With this data we have a pruned view of the blockchain, metadata about the chain we are in, and the compact UTXO set (the utreexo accumulator).

![](./img/chainstore.png)

*Figure 3: Diagram of the ChainStore trait.*

`ChainStore` also has an associated `Error` type for the methods:

Filename: pruned_utreexo/mod.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/mod.rs
#
pub trait ChainStore {
    type Error: DatabaseError;

    fn save_roots_for_block(&mut self, roots: Vec<u8>, height: u32) -> Result<(), Self::Error>;
    // ...
    # fn load_roots_for_block(&mut self, height: u32) -> Result<Option<Vec<u8>>, Self::Error>;
    #
    # fn load_height(&self) -> Result<Option<BestChain>, Self::Error>;
    #
    # fn save_height(&mut self, height: &BestChain) -> Result<(), Self::Error>;
    #
    # fn get_header(&self, block_hash: &BlockHash) -> Result<Option<DiskBlockHeader>, Self::Error>;
    #
    # fn get_header_by_height(&self, height: u32) -> Result<Option<DiskBlockHeader>, Self::Error>;
    #
    # fn save_header(&mut self, header: &DiskBlockHeader) -> Result<(), Self::Error>;
    #
    # fn get_block_hash(&self, height: u32) -> Result<Option<BlockHash>, Self::Error>;
    #
    # fn flush(&mut self) -> Result<(), Self::Error>;
    #
    # fn update_block_index(&mut self, height: u32, hash: BlockHash) -> Result<(), Self::Error>;
    #
    # fn check_integrity(&self) -> Result<(), Self::Error>;
}
```

Hence, implementations of `ChainStore` are free to use any error type as long as it implements `DatabaseError`. This is just a marker trait that can be automatically implemented on any `T: std::error::Error + std::fmt::Display`. This flexibility allows compatibility with different database implementations.

And that's all for this section! Next we will see two important types whose data is saved in the `ChainStore`: `BestChain` and `DiskBlockHeader`.

{{#quiz ../quizzes/ch02-01-the-chainstore-trait.toml}}
