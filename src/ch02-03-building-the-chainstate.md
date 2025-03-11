## Building the ChainState

The next step is building the `ChainState` struct, which validates blocks and updates the `ChainStore`.

Filename: pruned_utreexo/chain_state.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub struct ChainState<PersistedState: ChainStore> {
    inner: RwLock<ChainStateInner<PersistedState>>,
}
```

Note that the `RwLock` that we use to wrap `ChainStateInner` is not the one from the standard library but from the `spin` crate, thus allowing `no_std`.

> `std::sync::RwLock` relies on the OS to block and wake threads when the lock is available, while `spin::RwLock` uses a [spinlock](https://en.wikipedia.org/wiki/Spinlock) which does not require OS support for thread management, as the thread simply keeps running (and checking for lock availability) instead of sleeping.

The builder for `ChainState` has this signature:

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub fn new(
    chainstore: PersistedState,
    network: Network,
    assume_valid: AssumeValidArg,
) -> ChainState<PersistedState> {
```

The first argument is our `ChainStore` implementation, the second one is a `Network` enum, and thirdly the `AssumeValidArg` enum.

`Network` is an enum with four variants: `Bitcoin` (mainchain), `Testnet`, `Regtest` and `Signet`. It's declared in the _lib.rs_ of `floresta-chain`, along with conversions from and to `bitcoin::network::Network`, an identical enum from the `bitcoin` crate.

```rust
# // Path: floresta-chain/src/lib.rs
#
// This is the only thing implemented in lib.rs
pub enum Network {
    Bitcoin,
    Testnet,
    Regtest,
    Signet,
}

// impl From<bitcoin::network::Network> for Network { ... }

// impl From<Network> for bitcoin::network::Network { ... }
```

### The Assume-Valid Lore

The `assume_valid` argument refers to a Bitcoin Core option that allows nodes during IBD to assume the validity of scripts (mainly signatures) up to a certain block.

Nodes with this option enabled will still choose the most PoW chain (the best tip), and will only skip script validation if the `Assume-Valid` block is in that chain. Otherwise, if the `Assume-Valid` block is not in the best chain, they will validate everything.

> When users use the default `Assume-Valid` hash, hardcoded in the software, they aren't blindly trusting script validity. These hashes are reviewed through the same open-source process as other security-critical changes in Bitcoin Core, so the trust model is unchanged.

In Bitcoin Core, the hardcoded `Assume-Valid` block hash is included in _src/kernel/chainparams.cpp_.

Filename: pruned_utreexo/chain_state.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub enum AssumeValidArg {
    Disabled,
    Hardcoded,
    UserInput(BlockHash),
}
```

`Disabled` means the node verifies all scripts, `Hardcoded` means the node uses the default block hash that has been hardcoded in the software (and validated by maintainers, developers and reviewers), and `UserInput` means using a hash that the node runner provides, although the validity of the scripts up to this block should have been externally validated.

### Genesis and Assume-Valid Blocks

The first part of the body of `ChainState::new` (let's omit the `impl` block from now on):
```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub fn new(
    chainstore: PersistedState,
    network: Network,
    assume_valid: AssumeValidArg,
) -> ChainState<PersistedState> {
    let parameters = network.into();
    let genesis = genesis_block(&parameters);

    chainstore
        .save_header(&super::chainstore::DiskBlockHeader::FullyValid(
            genesis.header,
            0,
        ))
        .expect("Error while saving genesis");

    chainstore
        .update_block_index(0, genesis.block_hash())
        .expect("Error updating index");

    let assume_valid = ChainParams::get_assume_valid(network, assume_valid);
    // ...
    # ChainState {
        # inner: RwLock::new(ChainStateInner {
            # chainstore,
            # acc: Stump::new(),
            # best_block: BestChain {
                # best_block: genesis.block_hash(),
                # depth: 0,
                # validation_index: genesis.block_hash(),
                # alternative_tips: Vec::new(),
                # assume_valid_index: 0,
            # },
            # broadcast_queue: Vec::new(),
            # subscribers: Vec::new(),
            # fee_estimation: (1_f64, 1_f64, 1_f64),
            # ibd: true,
            # consensus: Consensus { parameters },
            # assume_valid,
        # }),
    # }
}
```

First, we use the `genesis_block` function from `bitcoin` to retrieve the genesis block based on the specified parameters, which are determined by our `Network`.

Then we save the genesis header into `chainstore`, which of course is `FullyValid` and has height 0. We also link the index 0 with the genesis block hash.

Finally, we get an `Option<BlockHash>` by calling the `ChainParams::get_assume_valid` function, which takes a `Network` and an `AssumeValidArg`.

Filename: pruned_utreexo/chainparams.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/chainparams.rs
#
// Omitted: impl ChainParams {

pub fn get_assume_valid(network: Network, arg: AssumeValidArg) -> Option<BlockHash> {
    match arg {
        // No assume-valid hash
        AssumeValidArg::Disabled => None,
        // Use the user-provided hash
        AssumeValidArg::UserInput(hash) => Some(hash),
        // Fetch the hardcoded values, depending on the network
        AssumeValidArg::Hardcoded => match network {
            Network::Bitcoin => Some(bhash!(
                "00000000000000000000569f4d863c27e667cbee8acc8da195e7e5551658e6e9"
            )),
            Network::Testnet => Some(bhash!(
                "000000000000001142ad197bff16a1393290fca09e4ca904dd89e7ae98a90fcd"
            )),
            Network::Signet => Some(bhash!(
                "0000003ed17b9c93954daab00d73ccbd0092074c4ebfc751c7458d58b827dfea"
            )),
            Network::Regtest => Some(bhash!(
                "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
            )),
        },
    }
}
```

The final part of `ChainState::new` just returns the instance of `ChainState` with the `ChainStateInner` initialized. We will see this initialization next.

{{#quiz ../quizzes/ch02-03-building-the-chainstate.toml}}
