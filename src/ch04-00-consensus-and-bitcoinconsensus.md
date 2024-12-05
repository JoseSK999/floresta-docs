# Consensus and bitcoinconsensus

In the previous chapter, we saw that the block validation process involves two associated functions from `Consensus`:

- `verify_block_transactions`: The last check performed inside `validate_block_no_acc`, after having validated the two merkle roots and height commitment.
- `update_acc`: Called inside `connect_block`, just after `validate_block_no_acc`, to verify the utreexo proof and get the updated accumulator.

The `Consensus` struct only holds a `parameters` field (as we saw [when we initialized ChainStateInner](ch02-04-initializing-chainstateinner.md#initial-chainstateinner-values)) and provides a few core consensus functions. In this chapter we are going to see the two mentioned functions and discuss the details of how we verify scripts.

Filename: pruned_utreexo/consensus.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/consensus.rs
#
pub struct Consensus {
    // The chain parameters are in the chainparams.rs file
    pub parameters: ChainParams,
}
```

## bitcoinconsensus

`Consensus::verify_block_transactions` is a critical part of Floresta, as it validates all the transactions in a block. One of the hardest parts for validation is checking the **script satisfiability**, that is, verifying whether the inputs can indeed spend the coins. It's also the most resource-intensive part of block validation, as it requires verifying many digital signatures.

Implementing a [Bitcoin script](https://en.bitcoin.it/wiki/Script) interpreter is challenging, and given the complexity of both C++ and Rust, we cannot be certain that it will always behave in the same way as `Bitcoin Core`. This is problematic because if our Rust implementation rejects a script that `Bitcoin Core` accepts, our node will fork from the network. It will treat subsequent blocks as invalid, halting synchronization with the chain and being unable to continue tracking the user balance.

Partly because of this reason, in 2015 the script validation logic of `Bitcoin Core` was extracted and placed into the [libbitcoin-consensus](https://github.com/libbitcoin/libbitcoin-consensus) library. This library includes 35 files that are identical to those of `Bitcoin Core`. Subsequently, the library API was bound to Rust in [rust-bitcoinconsensus](https://github.com/rust-bitcoin/rust-bitcoinconsensus), which serves as the `bitcoinconsensus` feature-dependency in the `bitcoin` crate.

If this feature is set, `bitcoin` provides a `verify_with_flags` method on `Transaction`, which performs the script validation by calling C++ code extracted from `Bitcoin Core`. Floresta uses this method to verify scripts.

> ### libbitcoinkernel
> 
> `bitcoinconsensus` handles only script validation and is maintained as a separate project from `Bitcoin Core`, with limited upkeep.
> 
> To address these shortcomings there's an ongoing effort within the `Bitcoin Core` community to extract the whole consensus engine into a library. This is known as the [libbitcoinkernel](https://github.com/bitcoin/bitcoin/issues/27587) project.
> 
> Once this is achieved, we should be able to drop _all the consensus code_ in Floresta and replace the `bitcoinconsensus` dependency with the Rust bindings for the new library. This would make Floresta safer and more reliable as a full node.
