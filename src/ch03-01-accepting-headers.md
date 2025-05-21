## Accepting Headers

The full `accept_header` method implementation for `ChainState` is below. To get read or write access to the `ChainStateInner` we use two macros, `read_lock` and `write_lock`.

In short, the method takes a `bitcoin::block::Header` (type alias `BlockHeader`) and accepts it on top of our chain of headers, or maybe reorgs if it's extending a better chain (i.e. switching to the new better chain). If there's an error it returns `BlockchainError`, which we mentioned in [The UpdatableChainstate Trait](ch01-02-chain-backend-api.md#the-updatablechainstate-trait) subsection from Chapter 1.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn accept_header(&self, header: BlockHeader) -> Result<(), BlockchainError> {
    debug!("Accepting header {header:?}");
    let disk_header = self.get_disk_block_header(&header.block_hash());

    match disk_header {
        Err(e @ BlockchainError::Database(_)) => {
            // If there's a database error we don't know if we already
            // have the header or not
            return Err(e);
        }
        Ok(found) => {
            // Possibly reindex to recompute the best_block field
            self.maybe_reindex(&found);
            // We already have this header
            return Ok(());
        }
        _ => (),
    }
    // The best block we know of
    let best_block = self.get_best_block()?;

    // Do validation in this header
    let block_hash = self.validate_header(&header)?;

    // Update our current tip
    if header.prev_blockhash == best_block.1 {
        let height = best_block.0 + 1;
        debug!("Header builds on top of our best chain");

        let mut inner = write_lock!(self);
        inner.best_block.new_block(block_hash, height);
        inner
            .chainstore
            .save_header(&super::chainstore::DiskBlockHeader::HeadersOnly(
                header, height,
            ))?;

        inner.chainstore.update_block_index(height, block_hash)?;
    } else {
        debug!("Header not in the best chain");
        self.maybe_reorg(header)?;
    }

    Ok(())
}
```

First, we check if we already have the header in our database. We query it with the `get_disk_block_header` method, which just wraps `ChainStore::get_header` in order to return `BlockchainError` (instead of `T: DatabaseError`).

If `get_disk_block_header` returns `Err` it may be because the header was not in the database or because there was a `DatabaseError`. In the latter case, we propagate the error.

#### We have the header

If we already have the header in our database we may reindex, which means recomputing the `BestChain` struct, and return `Ok` early.

> Reindexing updates the `best_block` field if it is not up-to-date with the disk headers (for instance, having headers up to the 105th, but `best_block` only referencing the 100th). This happens when the node is turned off or crashes before persisting the latest `BestChain` data.

#### We don't have the header

If we don't have the header, then we get the best block hash and height (with `BlockchainInterface::get_best_block`) and perform [a simple validation](ch03-01-accepting-headers.md#validate-header) on the header with `validate_header`. If validation passes, we _potentially update the current tip_.

- If the new header extends the previous best block:
    1. We update the `best_block` field, adding the new block hash and height.
    2. Then we call `save_header` and `update_block_index` to update the database (or the `HashMap` caches if we use `KvChainStore`).
- If the header doesn't extend the current best chain, we may [reorg](ch03-02-reorging-the-chain.md) if it extends a better chain.

### Reindexing

During IBD, headers arrive rapidly, making it pointless to write the `BestChain` data to disk for every new header. Instead, we update the `ChainStateInner.best_block` field and only persist it occasionally, avoiding redundant writes that would instantly be overridden.

But there is a possibility that the node _is shut down or crashes_ before `save_height` is called (or before the pending write is completed) and after the headers have been written to disk. In this case we can recompute the last `BestChain` data by going through the headers on disk. This recovery process is handled by the `reindex_chain` method within `maybe_reindex`.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn maybe_reindex(&self, potential_tip: &DiskBlockHeader) {

    // Check if the disk header is an unvalidated block in the best chain
    if let DiskBlockHeader::HeadersOnly(_, height) = potential_tip {
        let best_height = self
            .get_best_block()
            .expect("infallible: in-memory BestChain is initialized")
            .0;

        // If the best chain height is lower, it needs to be updated
        if *height > best_height {
            let best_chain = self.reindex_chain();
            write_lock!(self).best_block = best_chain;
        }
    }
}
```

We call `reindex_chain` if _disk header's height > best_block's height_, as it means that `best_block` is not up-to-date with the headers on disk.

### Validate Header

The `validate_header` method takes a `BlockHeader` and performs the following checks:

#### Check the header chain
- Retrieve the previous `DiskBlockHeader`. If not found, return `BlockchainError::BlockNotPresent` or `BlockchainError::Database`.
- If the previous `DiskBlockHeader` is marked as `Orphan` or `InvalidChain`, return `BlockchainError::BlockValidation`.

#### Check the PoW

- Use the `get_next_required_work` method to compute the expected PoW target and compare it with the header's actual target. If the actual target is easier, return `BlockchainError::BlockValidation`.
- Verify the PoW against the target using a `bitcoin` method. If verification fails, return `BlockchainError::BlockValidation`.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn validate_header(&self, block_header: &BlockHeader) -> Result<BlockHash, BlockchainError> {
    # let prev_block = self.get_disk_block_header(&block_header.prev_blockhash)?;
    # let height = prev_block
        # .height()
        # .ok_or(BlockValidationErrors::BlockExtendsAnOrphanChain)?
        # + 1;
    // ...

    // Check pow
    let expected_target = self.get_next_required_work(&prev_block, height, block_header);

    let actual_target = block_header.target();
    if actual_target > expected_target {
        return Err(BlockValidationErrors::NotEnoughPow)?;
    }

    let block_hash = block_header
        .validate_pow(actual_target)
        .map_err(|_| BlockValidationErrors::NotEnoughPow)?;
    Ok(block_hash)
}
```

A block header passing this validation will not make the block itself valid, but we can use this to build the chain of headers with verified PoW.
