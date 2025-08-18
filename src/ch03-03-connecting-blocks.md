## Connecting Blocks

Great! At this point we should have a sense of the inner workings of `accept_headers`. Let's now understand the `connect_block` method, which performs the actual block validation and updates the `ChainStateInner` fields and database. This function is meant to increase our chain validation index, and so it requires to be called on the right block (i.e., the next one to validate).

`connect_block` takes a `Block`, an UTXO set inclusion `Proof` from `rustreexo`, the UTXOs to spend (stored with metadata in a custom floresta type called `UtxoData`) and the hashes from said outputs. If result is `Ok` the function returns the height.

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
        DiskBlockHeader::FullyValid(_, height) => {
            let validation_index = self.get_validation_index()?;

            // If this block is not our validation index, but the caller is trying to connect
            // it, this is a logical error, and we will have spurious errors, specially with
            // invalid proof. They don't mean the block is invalid, just that we are using the
            // wrong accumulator, since we are not processing the right block.
            if height != validation_index {
                return Err(BlockValidationErrors::BlockDoesntExtendTip)?;
            }

            // If this block is our validation index, but it's fully valid, this clearly means
            // there was some corruption of our state. If we don't process this block, we will
            // be stuck forever.
            # //
            # // Note: You may think "just kick the validation index one block further and we are
            # // good". But this is not the case, because we still need to update our
            # // accumulator. Otherwise, the next block will always have an invalid proof
            # // (because the accumulator is not updated).
            height
        },

        // Our called tried to connect_block on a block that is not the next one in our chain
        DiskBlockHeader::Orphan(_)
        | DiskBlockHeader::AssumedValid(_, _) // this will be validated by a partial chain
        | DiskBlockHeader::InFork(_, _)
        | DiskBlockHeader::InvalidChain(_) => return Err(BlockValidationErrors::BlockExtendsAnOrphanChain)?,

        DiskBlockHeader::HeadersOnly(_, height) => {
            let validation_index = self.get_validation_index()?;

            // In case of a `HeadersOnly` block, we need to check if the height is
            // the next one after the validation index. If not, we would be trying to
            // connect a block where our accumulator isn't the right one. So the proof will
            // always be invalid.
            if height != validation_index + 1 {
                return Err(BlockValidationErrors::BlockDoesntExtendTip)?;
            }

            height
        }
    };

    // Clone inputs only if a subscriber wants spent utxos
    let inputs_for_notifications = self
        .inner
        .read()
        .subscribers
        .iter()
        .any(|subscriber| subscriber.wants_spent_utxos())
        .then(|| inputs.clone());

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

    if !self.is_in_ibd() || height % 100_000 == 0 {
        self.flush()?;
    }

    // Notify others we have a new block
    self.notify(block, height, inputs_for_notifications.as_ref());
    Ok(height)
}
```

When we call `connect_block`, the header should already be stored on disk, as `accept_header` is called first. Then we will verify we are calling the function for the right block.

If the header is `FullyValid` it means we already validated the block, and we only try to re-connect the block if it's the last validated block (i.e., the validation index), which could be needed if some of our data was lost. Else if the header is `Orphan`, `AssumeValid`, `InFork` or `InvalidChain` we return an error, as this means our block is not mainchain or doesn't require validation.

If header is `HeadersOnly`, meaning the block is an unvalidated mainchain block, we will check it is the next one to validate. Thus, if we validated up to block _h_, then we must call `connect_block` for block _h + 1_ (this is because we can only use the accumulator at height _h_ to validate the block _h + 1_).

So, when `block` is the next block to validate, or it is the validation index, we go on to validate it using `validate_block_no_acc`, and then the `Consensus::update_acc` function, which verifies the inclusion proof against the accumulator and returns the updated accumulator.

After this, we have fully validated the block! The next steps in `connect_block` are updating the state and notifying the block to subscribers.

### Post-Validation

After block validation we call `update_view` to mark the disk header as `FullyValid` (`ChainStore::save_header`), update the block hash index (`ChainStore::update_block_index`) and also update `ChainStateInner.acc` and the validation index of `best_block`.

Then, we call `UpdatableChainstate::flush` _every 100,000 blocks during IBD_ or for _each new block once synced_. In order, this method invokes:
1. `save_acc`, which serializes the accumulator and calls `ChainStore::save_roots`
2. `ChainStore::save_height`
3. `ChainStore::flush`, to immediately flush to disk all pending writes

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn flush(&self) -> Result<(), BlockchainError> {
    let mut inner = write_lock!(self);
    let best_block = inner.best_block.clone();

    inner.chainstore.save_height(&best_block)?;
    inner.chainstore.flush()?;

    Ok(())
}
```

Last of all, we `notify` the new validated block to subscribers.
