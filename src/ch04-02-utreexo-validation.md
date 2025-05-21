## Utreexo Validation

In the previous section we have seen the `Consensus::verify_block_transactions` function. It was taking a `utxos` argument, used to verify that each transaction input _satisfies the script_ of the UTXO it spends, and that transactions _spend no more than the sum of input amounts_.

However, we have yet to **verify that these `utxos` actually exist in the UTXO set**, i.e. check that nobody is spending coins out of thin air. That's what we are going to do inside `Consensus::update_acc`, and get the updated UTXO set accumulator, with spent UTXOs removed and new ones added.

> Recall that `Stump` is the type of our accumulator, coming from the `rustreexo` crate. `Stump` represents the merkle roots of a forest where leaves are UTXO hashes.

![](./img/utreexo-forest.png)

*Figure 4: A visual depiction of the utreexo forest. To prove that UTXO `4` is part of the set we provide the hash of UTXO `3` and `h1`. With this data we can re-compute the `h5` root, which must be identical. Credit: [original utreexo post](https://medium.com/interdax/utreexo-compressing-fully-validating-bitcoin-nodes-4174d95e0626).*

This function first verifies we are not spending any of the two historical unspendable UTXOs according to [BIP 30](https://github.com/bitcoin/bips/blob/master/bip-0030.mediawiki). In the early days of Bitcoin there were two occurrences of duplicate transaction IDs, which override the previous UTXO with the same transaction ID. Because utreexo leaf hashes commit to more data, we don't see the previous UTXO as overridden, so we need to enforce it is unspendable manually.

```rust
# // Path: floresta-chain/src/pruned_utreexo/consensus.rs
#
// Omitted: impl Consensus {

pub fn update_acc(
    acc: &Stump,
    block: &Block,
    height: u32,
    proof: Proof,
    del_hashes: Vec<sha256::Hash>,
) -> Result<Stump, BlockchainError> {
    let block_hash = block.block_hash();

    // Check if there is a spend of an unspendable UTXO (BIP30)
    if Self::contains_unspendable_utxo(&del_hashes) {
        return Err(BlockValidationErrors::UnspendableUTXO)?;
    }

    // Convert to BitcoinNodeHash, from rustreexo
    let del_hashes: Vec<_> = del_hashes.into_iter().map(Into::into).collect();

    let adds = udata::proof_util::get_block_adds(block, height, block_hash);

    // Update the accumulator
    let acc = acc.modify(&adds, &del_hashes, &proof)?.0;
    Ok(acc)
}
```

Then we get the new leaf hashes (the hashes of newly created UTXOs in the block) by calling `udata::proof_util::get_block_adds`. This function returns the new leaves to add to the accumulator, which exclude two cases:
1. Created UTXOs that are provably unspendable (i.e., an OP_RETURN output or any output with a script larger than 10,000 bytes).
2. Created UTXOs spent within the same block.

Finally, we get the updated `Stump` using its `modify` method, provided the leaves to add, the leaves to remove and the proof of inclusion for the latter. This method both verifies the proof and generates the new accumulator.
