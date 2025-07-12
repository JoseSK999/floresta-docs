## Reorging the Chain

In the `accept_header` method we have seen that, when receiving a header that doesn't extend the best chain, we may reorg. This is done with the `maybe_reorg` method.

We have to choose between the two branches, represented by:
- `branch_tip`: The last header from the alternative chain.
- `current_tip`: The last header from the current best chain.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn maybe_reorg(&self, branch_tip: BlockHeader) -> Result<(), BlockchainError> {
    let current_tip = self.get_block_header(&self.get_best_block()?.1)?;
    self.check_branch(&branch_tip)?;

    let current_work = self.get_branch_work(&current_tip)?;
    let new_work = self.get_branch_work(&branch_tip)?;
    // If the new branch has more work, it becomes the new best chain
    if new_work > current_work {
        self.reorg(branch_tip)?;
        return Ok(());
    }
    // If the new branch has less work, we just store it as an alternative branch
    // that might become the best chain in the future.
    self.push_alt_tip(&branch_tip)?;

    let parent_height = self.get_ancestor(&branch_tip)?.try_height()?;
    self.update_header(&DiskBlockHeader::InFork(branch_tip, parent_height + 1))?;

    Ok(())
}
```

We first call the `check_branch` method to check if we know all the `branch_tip` ancestors. In other words, we check if `branch_tip` is indeed part of a branch, which requires that no ancestor is `Orphan`.

Then we get the work in each chain tip with `get_branch_work` and do the following:
- We reorg to the `branch_tip` if it has more work, and return `Ok` early.
- Else if `branch_tip` doesn't have more work we push its hash to `best_block.alternative_tips` via the `push_alt_tip` method and save the header as `InFork`. 

The `push_alt_tip` method just checks if the `branch_tip` parent hash is in `alternative_tips` to remove it, as it's no longer the tip of the branch. Then we simply push the `branch_tip` hash.

### Reorg

Let's now dig into reorg logic, with `reorg`. We start by querying the best block hash and use it to query its header. Then we get the header where the branch forks out with `find_fork_point`.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn reorg(&self, new_tip: BlockHeader) -> Result<(), BlockchainError> {
    let current_best_block = self.get_block_header(&self.get_best_block()?.1)?;
    let fork_point = self.find_fork_point(&new_tip)?;

    self.mark_chain_as_inactive(&current_best_block, fork_point.block_hash())?;
    self.mark_chain_as_active(&new_tip, fork_point.block_hash())?;

    let validation_index = self.get_last_valid_block(&new_tip)?;
    let depth = self.get_chain_depth(&new_tip)?;

    self.change_active_chain(&new_tip, validation_index, depth);
    self.reorg_acc(&fork_point)?;

    Ok(())
}
```

We use `mark_chain_as_inactive` and `mark_chain_as_active` to update the disk data (i.e., marking the previous `InFork` headers as `HeadersOnly` and vice versa, and linking the height indexes to the new branch block hashes).

Then we invoke `get_last_valid_block` and `get_chain_depth` to obtain said data from a branch, provided the branch header tip.

> Note that we don't validate forks unless they become the best chain, so in this case the last validated block is the last common block between the two branches.

With this data we call `change_active_chain` to update the `best_block` field. We also call `reorg_acc` to roll back to the saved accumulator for the new last validated block, which is needed to proceed with the new branch validation.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
fn change_active_chain(&self, new_tip: &BlockHeader, last_valid: BlockHash, depth: u32) {
    let mut inner = self.inner.write();
    inner.best_block.best_block = new_tip.block_hash();
    inner.best_block.validation_index = last_valid;
    inner.best_block.depth = depth;
}
```
