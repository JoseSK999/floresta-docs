## Connecting Blocks

Great! At this point we should have a sense of the inner workings of `accept_headers`. Let's now understand the `connect_block` method, which performs the actual block validation and updates the `ChainStateInner` fields and database.

`connect_block` takes a `Block`, an UTXO set inclusion `Proof` from `rustreexo`, the outputs to spend from the UTXO set (stored with metadata in a custom floresta type called `UtxoData`) and the hashes from said outputs. If result is `Ok` the function returns the height.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn connect_block(
    &self,
    block: &Block,
    proof: Proof,
    inputs: HashMap<OutPoint, UtxoData>,
    del_hashes: Vec<sha256::Hash>,
) -> Result<u32, BlockchainError> {
    let header = self.get_disk_block_header(&block.block_hash())?;
    let height = match header {
        DiskBlockHeader::FullyValid(_, height) => return Ok(height),
        // If it's valid or orphan, we don't validate
        DiskBlockHeader::Orphan(_)
        | DiskBlockHeader::AssumedValid(_, _) // this will be validated by a partial chain
        | DiskBlockHeader::InFork(_, _)
        | DiskBlockHeader::InvalidChain(_) => return Ok(0),
        DiskBlockHeader::HeadersOnly(_, height) => height,
    };

    // Check if this block is the next one in our chain, if we try
    // to add them out-of-order, we'll have consensus issues with our
    // accumulator
    let expected_height = self.get_validation_index()? + 1;
    if height != expected_height {
        return Ok(height);
    }

    self.validate_block_no_acc(block, height, inputs)?;
    let acc = Consensus::update_acc(&self.acc(), block, height, proof, del_hashes)?;

    self.update_view(height, &block.header, acc)?;

    info!(
        "New tip! hash={} height={height} tx_count={}",
        block.block_hash(),
        block.txdata.len()
    );
    #
    # #[cfg(feature = "metrics")]
    # metrics::get_metrics().block_height.set(height.into());

    if !self.is_in_ibd() || height % 10_000 == 0 {
        self.flush()?;
    }

    // Notify others we have a new block
    self.notify(block, height);
    Ok(height)
}
```

When we call `connect_block`, the header should already be stored on disk, as `accept_header` is called first.

If the header is `FullyValid` it means we already validated the block, so we can return `Ok` early. Else if the header is `Orphan`, `AssumeValid`, `InFork` or `InvalidChain` we don't validate and return `Ok` with height 0.

> Recall that `InvalidChain` doesn't mean our blockchain backend validated the block with a false result. Rather it means the backend was told to consider it invalid with `BlockchainInterface::invalidate_block`.

If header is `HeadersOnly` we get the height and continue. If this block, however, is not next one to validate, we return early again without validating the block. This is because we can only use the accumulator at height _h_ to validate the block _h + 1_.

When `block` is the next block to validate, we finally use `validate_block_no_acc`, and then the `Consensus::update_acc` function, which verifies the inclusion proof against the accumulator and returns the updated accumulator.

After this, we have fully validated the block! The next steps in `connect_block` are updating the state and notifying the block to subscribers.

### Post-Validation

After block validation we call `update_view` to mark the disk header as `FullyValid` (`ChainStore::save_header`), update the block hash index (`ChainStore::update_block_index`) and also update `ChainStateInner.acc` and the validation index of `best_block`.

Then, we call `UpdatableChainstate::flush` _every 10,000 blocks during IBD_ or for _each new block once synced_. In order, this method invokes:
1. `save_acc`, which serializes the accumulator and calls `ChainStore::save_roots`
2. `ChainStore::save_height`
3. `ChainStore::flush`, to immediately flush to disk all pending writes

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn flush(&self) -> Result<(), BlockchainError> {
    self.save_acc()?;
    let inner = read_lock!(self);
    inner.chainstore.save_height(&inner.best_block)?;
    inner.chainstore.flush()?;
    Ok(())
}
```

> Note that this is the only time we persist the roots and height (best chain data), and it is the only time we persist the headers and index data if we use `KvChainStore` as store.

Last of all, we `notify` the new validated block to subscribers.
