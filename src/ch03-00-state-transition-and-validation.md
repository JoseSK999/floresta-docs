# State Transition and Validation

With the `ChainState` struct built, the foundation for running a node, we can now dive into the methods that **validate and apply state transitions**.

`ChainState` has 4 `impl` blocks (all located in _pruned_utreexo/chain_state.rs_):
- The `BlockchainInterface` trait implementation
- The `UpdatableChainstate` trait implementation
- The implementation of other methods and associated functions like `ChainState::new`
- The conversion from `ChainStateBuilder` (builder type located in _pruned_utreexo/chain_state_builder.rs_) to `ChainState`

The entry point to the state transition and validation logic are the `accept_header` and `connect_block` methods from `UpdatableChainstate`. As we have explained previously, the first step in the IBD is accepting headers, so we will start with that.
