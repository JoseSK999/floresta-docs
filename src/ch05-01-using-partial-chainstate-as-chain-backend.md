## Using PartialChainState as Chain Backend

As you may be guessing, `PartialChainState` is an alternative chain validation backend made to leverage the parallel sync feature we have described (out-of-order validation).

In the [Chain Backend API](ch01-02-chain-backend-api.md) section from Chapter 1 we saw that the `UpdatableChainstate` trait requires a `get_partial_chain` method.

```rust
# // Path: floresta-chain/src/pruned_utreexo/mod.rs
#
pub trait UpdatableChainstate {
    # fn connect_block(
        # &self,
        # block: &Block,
        # proof: Proof,
        # inputs: HashMap<OutPoint, TxOut>,
        # del_hashes: Vec<sha256::Hash>,
    # ) -> Result<u32, BlockchainError>;
    #
    # fn switch_chain(&self, new_tip: BlockHash) -> Result<(), BlockchainError>;
    #
    # fn accept_header(&self, header: BlockHeader) -> Result<(), BlockchainError>;
    #
    # fn handle_transaction(&self) -> Result<(), BlockchainError>;
    #
    # fn flush(&self) -> Result<(), BlockchainError>;
    #
    # fn toggle_ibd(&self, is_ibd: bool);
    #
    # fn invalidate_block(&self, block: BlockHash) -> Result<(), BlockchainError>;
    #
    # fn mark_block_as_valid(&self, block: BlockHash) -> Result<(), BlockchainError>;
    #
    # fn get_root_hashes(&self) -> Vec<BitcoinNodeHash>;
    #
    // ...
    fn get_partial_chain(
        &self,
        initial_height: u32,
        final_height: u32,
        acc: Stump,
    ) -> Result<PartialChainState, BlockchainError>;
    // ...
    #
    # fn mark_chain_as_assumed(&self, acc: Stump, tip: BlockHash) -> Result<bool, BlockchainError>;
    #
    # fn get_acc(&self) -> Stump;
}
```

The arguments that the method takes are indeed the block interval to validate and the accumulator at the start of the interval.

Just like `ChainState`, `PartialChainState` wraps an inner type which holds the actual data. However, instead of maintaining synchronization primitives, `PartialChainState` assumes that only a single worker (thread or async task) will hold ownership at any given time. This design expects workers to operate independently, with each validating its assigned partial chain. Once all partial chains are validated, we can transition to the `ChainState` backend.

Filename: pruned_utreexo/partial_chain.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/partial_chain.rs
#
pub(crate) struct PartialChainStateInner {
    /// The current accumulator state, it starts with a hardcoded value and
    /// gets checked against the result of the previous partial chainstate.
    pub(crate) current_acc: Stump,
    # /// The block headers in this interval, we need this to verify the blocks
    # /// and to build the accumulator. We assume this is sorted by height, and
    # /// should contain all blocks in this interval.
    pub(crate) blocks: Vec<BlockHeader>,
    /// The height we are on right now, this is used to keep track of the progress
    /// of the sync.
    pub(crate) current_height: u32,
    # /// The height we are syncing up to, trying to push more blocks than this will
    # /// result in an error.
    pub(crate) final_height: u32,
    /// The error that occurred during validation, if any. It is here so we can
    /// pull that afterward.
    pub(crate) error: Option<BlockValidationErrors>,
    /// The consensus parameters, we need this to validate the blocks.
    pub(crate) consensus: Consensus,
    /// Whether we assume the signatures in this interval as valid, this is used to
    /// speed up syncing, by assuming signatures in old blocks are valid.
    pub(crate) assume_valid: bool,
}
```

We can see that `PartialChainStateInner` has an `assume_valid` field. By combining the parallel sync with `Assume-Valid` we get a huge IBD speedup, with virtually no security trade-off. Most of the expensive script validations are skipped, while the remaining checks are performed in parallel and without disk access. In this IBD configuration, the primary bottleneck is likely network latency.

In the _pruned_utreexo/partial_chain.rs_ file, we also find the `BlockchainInterface` and `UpdatableChainstate` implementations for `PartialChainState`. These implementations are similar to those for `ChainState`, but many methods remain unimplemented because `PartialChainState` is designed specifically for IBD and operates with limited data. For instance:

```rust
# // Path: floresta-chain/src/pruned_utreexo/partial_chain.rs
#
fn accept_header(&self, _header: BlockHeader) -> Result<(), BlockchainError> {
    unimplemented!("partialChainState shouldn't be used to accept new headers")
}
```

Finally, there are very simple methods to get data from the status of validation of the partial chain:

```rust
# // Path: floresta-chain/src/pruned_utreexo/partial_chain.rs
#
/// Returns whether any block inside this interval is invalid
pub fn has_invalid_blocks(&self) -> bool {
    self.inner().error.is_some()
}
```

## Moving On

Now that we’ve explored the power of utreexo and how Floresta leverages it, along with:

- The structure of `floresta-chain`, including `ChainState` and `PartialChainState`.
- The use of the `ChainStore` trait for `ChainState`.
- The consensus validation process.

We are now ready to fully delve into `floresta-wire`: **the chain provider**.
