# Advanced Chain Validation Methods

Before moving on from `floresta-chain` to `floresta-wire`, it's time to understand some advanced validation methods and another `Chain` backend provided by Floresta: the `PartialChainState`.

Although similar to `ChainState`, `PartialChainState` focuses on validating a limited range of blocks rather than the entire chain. This design is closely linked to concepts like **UTXO snapshots** (precomputed UTXO states at specific blocks) and, in particular, **out-of-order validation**, which we’ll explore below.

## Out-of-Order Validation

One of the most powerful features enabled by utreexo is out-of-order validation, which allows **block intervals to be validated independently** if we know the utreexo roots at the start of each interval.

In traditional IBDs, block validation is inherently sequential: a block at height `h` depends on the UTXO set resulting from block `h - 1`, which in turn depends on `h - 2`, and so forth. However, with UTXO set snapshots for specific blocks, validation can become non-linear.

![](./img/out-of-order-validation.png)

*Figure 5: Visual explanation of three block intervals, starting at block `1`, `100,001`, and `200,001`, that can be validated in parallel if we have the UTXO sets for those blocks. Credit: [original post from Calvin Kim](https://blog.bitmex.com/out-of-order-block-validation-with-utreexo-accumulators/).*

This process remains fully trustless because, at the end, we verify that the resulting UTXO set from one interval matches the UTXO set snapshot used to start the next. For example, in the image, the UTXO set after block `100,000` must match the set used for the interval beginning at block `100,001`, and so on.

Ultimately, the sequential nature of block validation is preserved. The worst outcome is wasting resources if the UTXO snapshots are incorrect, so it's still important to obtain these snapshots from a reliable source, such as hardcoded values within the software or reputable peers.

> #### Out-of-Order Validation Without Utreexo
>
> Out-of-order validation is technically possible without utreexo, but it would require entire UTXO sets for each interval, which would take many gigabytes.
> 
> Utreexo makes this feasible with compact accumulators, avoiding the need for full UTXO set storage and frequent disk reads. Instead, spent UTXOs are fetched on demand from the network, along with their inclusion proofs.

Essentially, we are trading disk operations for hash computations (by verifying merkle proofs and updating roots), along with a slightly higher network data demand. In other words, utreexo enables parallel validation while avoiding the bottleneck of slow disk access.

## Trusted UTXO Set Snapshots

A related but slightly different concept is the `Assume-Utxo` feature in `Bitcoin Core`, which hardcodes a trusted, recent UTXO set hash. When a new node starts syncing, it downloads the corresponding UTXO set from the network, verifies its hash against the hardcoded value, and temporarily assumes it to be valid. Starting from this snapshot, the node can quickly sync to the chain tip (e.g., if the snapshot is from block `850,000` and the tip is at height `870,000`, only 20,000 blocks need to be validated to get a synced node).

This approach bypasses most of the IBD time, enabling rapid node synchronization while still silently completing IBD in the background to fully validate the UTXO set snapshot. It builds on [the `Assume-Valid` concept](ch02-03-building-the-chainstate.md#the-assume-valid-lore), relying on the open-source process to ensure the correctness of hardcoded UTXO set hashes.

This idea, adapted to Floresta, is what we call `Assume-Utreexo`, a hardcoded UTXO snapshot in the form of utreexo roots. These hardcoded values are located in _pruned_utreexo/chainparams.rs_, alongside the `Assume-Valid` hashes.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chainparams.rs
#
impl ChainParams {
    pub fn get_assume_utreexo(network: Network) -> Result<AssumeUtreexoValue, BlockchainError> {
        let genesis = genesis_block(Params::new(network));
        match network {
            Network::Bitcoin => Ok(AssumeUtreexoValue {
                block_hash: bhash!(
                    "0000000000000000000239f2b7f982df299193bdd693f499e6b893d8276ab7ce"
                ),
                height: 902967,
                roots: acchashes![
                    # "bd53eef66849c9d3ca13b62ce694030ac4d4b484c6f490f473b9868a7c5df2e8",
                    # "993ffb1782db628c18c75a5edb58d4d506167d85ca52273e108f35b73bb5b640",
                    # "36d8c4ba5176c816bdae7c4119d9f2ea26a1f743f5e6e626102f66a835eaac6d",
                    # "4c93092c1ecd843d2b439365609e7f616fe681de921a46642951cb90873ba6ce",
                    # "9b4435987e18e1fe4efcb6874bba5cdc66c3e3c68229f54624cb6343787488a4",
                    # "ab1e87c4066bf195fa7b8357874b82de4fa09ddba921499d6fc73aa133200505",
                    # "8f8215e284dbce604988755ba3c764dbfa024ae0d9659cd67b24742f46360849",
                    # "09b5057a8d6e1f61e93baf474220f581bd1a38d8a378dacb5f7fdec532f21e00",
                    # "a331072d7015c8d33a5c17391264a72a7ca1c07e1f5510797064fced7fbe591d",
                    # "c1c647289156980996d9ea46377e8c1b7e5c05940730ef8c25c0d081341221b5",
                    # "330115a495ed14140cd785d44418d84b872480d293972abd66e3325fdc78ac93",
                    # "b1d7a488e1197908efb2091a3b750508cb2fc495d2011bf2c34c5ae2d40bd2a5",
                    # "3b3b2e51ad96e1ae8ce468c7947b8aa2b41ecb400a32edec3dbcfe5ddb9aca50",
                    # "9d852775775f4c1e4a150404776a6b22569a0fe31f2e669fd3b31a0f70072800",
                    # "8e5f6a92169ad67b3f2682f230e2a62fc849b0a47bc36af8ce6cae24a5343126",
                    # "6dbd2925f8aa0745ac34fc9240ce2a7ef86953fc305c6570ef580a0763072bbe",
                    # "8121c38dcb37684c6d50175f5fd2695af3b12ce0263d20eb7cc503b96f7dba0d",
                    # "f5d8b30dd2038e1b3a5ced7a30c961e230270020c336fb649d0a9e169f11b876",
                    # "0466bd4eb9e7be5b8870e97d2a66377525391c16f15dbcc3833853c8d3bae51e",
                    # "976184c55f74cbb780938a20e2a5df2791cf51e712f68a400a6b024c77ad78e4",
                ]
                .to_vec(),
                leaves: 2860457445,
            }),
            Network::Testnet => Ok(AssumeUtreexoValue {
                // ...
                # block_hash: genesis.block_hash(),
                # height: 0,
                # leaves: 0,
                # roots: Vec::new(),
            }),
            Network::Testnet4 => Ok(AssumeUtreexoValue {
                // ...
                # block_hash: genesis.block_hash(),
                # height: 0,
                # leaves: 0,
                # roots: Vec::new(),
            }),
            Network::Signet => Ok(AssumeUtreexoValue {
                // ...
                # block_hash: genesis.block_hash(),
                # height: 0,
                # leaves: 0,
                # roots: Vec::new(),
            }),
            Network::Regtest => Ok(AssumeUtreexoValue {
                // ...
                # block_hash: genesis.block_hash(),
                # height: 0,
                # leaves: 0,
                # roots: Vec::new(),
            }),
            network => Err(BlockchainError::UnsupportedNetwork(network)),
        }
    }
    // ...
    #
    # pub fn get_assume_valid(
        network: Network,
        arg: AssumeValidArg,
    ) -> Result<Option<BlockHash>, BlockchainError> {
        # match arg {
            # AssumeValidArg::Disabled => Ok(None),
            # AssumeValidArg::UserInput(hash) => Ok(Some(hash)),
            # AssumeValidArg::Hardcoded => match network {
                # Network::Bitcoin => Ok(Some(bhash!(
                    # "00000000000000000001ff36aef3a0454cf48887edefa3aab1f91c6e67fee294"
                # ))),
                # Network::Testnet => Ok(Some(bhash!(
                    # "000000007df22db38949c61ceb3d893b26db65e8341611150e7d0a9cd46be927"
                # ))),
                # Network::Testnet4 => Ok(Some(bhash!(
                    # "0000000000335c2895f02ebc75773d2ca86095325becb51773ce5151e9bcf4e0"
                # ))),
                # Network::Signet => Ok(Some(bhash!(
                    # "000000084ece77f20a0b6a7dda9163f4527fd96d59f7941fb8452b3cec855c2e"
                # ))),
                # Network::Regtest => Ok(Some(bhash!(
                    # "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
                # ))),
                # network => Err(BlockchainError::UnsupportedNetwork(network)),
            # },
        # }
    # }
}
```

If you click _see more_, you will notice we have 17 utreexo roots there, and this is the accumulator for more than 2 billion UTXOs!

### PoW Fraud Proofs Sync

PoW Fraud Proofs Sync is yet another technique to speedup node synchronization, which was ideated by [Ruben Somsen](https://gnusha.org/pi/bitcoindev/CAPv7TjYspkc1M=TKmBK8k0Zy857=bR7jSTarRDCr_5m2ktYHDQ@mail.gmail.com/). It is similar in nature to running a light or SPV client, but with almost the security of a full node. This is the most powerful IBD optimization that Floresta implements, alongside `Assume-Utreexo`.

The idea that underlies this type of sync is that we can treat blockchain forks as potential-fraud proofs. If a miner creates an invalid block (violating consensus rules), honest miners will not mine on top of such block. Instead, honest miners will fork the chain by mining an alternative, valid block at the same height.

As long as a small fraction of miners remains honest and produces at least one block, a non-validating observer can interpret blockchain forks as indicators of _potentially invalid blocks_, and will always catch any invalid block.

The PoW Fraud Proof sync process begins by **identifying the most PoW chain**, which only requires downloading block headers:

- _If no fork is found_, the node assumes the most PoW chain is valid and begins validating blocks starting close to the chain tip.
- _If a fork is found_, this suggests a potential invalid block in the most PoW chain (prompting honest miners to fork away). The node downloads and verifies the disputed block, which requires using the UTXO accumulator for that block. If valid, the node continues following the most PoW chain; if invalid, it switches to the alternative branch.

This method bypasses almost entirely the IBD verification while maintaining security. It relies on a small minority of honest hashpower (e.g., ~1%) to fork away from invalid chains, which we use to detect the invalid blocks.

> In short, **PoW Fraud Proofs Sync requires at least some valid blocks to be produced for invalid ones to be detected**, whereas a regular full node, by validating every block, can detect invalid blocks even with 0% honest miners (though in that extreme case, the entire network would be in serious trouble 😄).

Hence, a PoW Fraud Proof synced node is vulnerable only when the Bitcoin chain is halted for an extended period of time, which would be catastrophic anyway. Check out [this blog post](https://blog.dlsouza.lol/2023/09/28/pow-fraud-proof.html) by Davidson for more details.
