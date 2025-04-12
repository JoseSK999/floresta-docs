## Transaction Validation

Let's now dive into `Consensus::verify_block_transactions`, to see how we verify the transactions in a block. As we saw in the [Block Validation](ch03-04-block-validation.md) section from last chapter, this function takes the height, the UTXOs to spend, the spending transactions, the current subsidy, the `verify_script` boolean (which was only true when we are not in the `Assume-Valid` range) and the validation flags.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub fn validate_block_no_acc(
    &self,
    block: &Block,
    height: u32,
    inputs: HashMap<OutPoint, UtxoData>,
) -> Result<(), BlockchainError> {
    # if !block.check_merkle_root() {
        # return Err(BlockValidationErrors::BadMerkleRoot)?;
    # }
    #
    # let bip34_height = self.chain_params().params.bip34_height;
    # // If bip34 is active, check that the encoded block height is correct
    # if height >= bip34_height && self.get_bip34_height(block) != Some(height) {
        # return Err(BlockValidationErrors::BadBip34)?;
    # }
    #
    # if !block.check_witness_commitment() {
        # return Err(BlockValidationErrors::BadWitnessCommitment)?;
    # }
    #
    # if block.weight().to_wu() > 4_000_000 {
        # return Err(BlockValidationErrors::BlockTooBig)?;
    # }
    #
    # // Validate block transactions
    # let subsidy = read_lock!(self).consensus.get_subsidy(height);
    # let verify_script = self.verify_script(height);
    // ...
    #[cfg(feature = "bitcoinconsensus")]
    let flags = self.get_validation_flags(height, block.header.block_hash());
    #[cfg(not(feature = "bitcoinconsensus"))]
    let flags = 0;
    Consensus::verify_block_transactions(
        height,
        inputs,
        &block.txdata,
        subsidy,
        verify_script,
        flags,
    )?;
    Ok(())
}
```

### Validation Flags

The validation flags were returned by `get_validation_flags` based on the current height and block hash, and they are of type `core::ffi::c_uint`: a foreign function interface type used for the C++ bindings.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
// Omitted: impl<PersistedState: ChainStore> ChainState<PersistedState> {

fn get_validation_flags(&self, height: u32, hash: BlockHash) -> c_uint {
    let chain_params = &read_lock!(self).consensus.parameters;

    if let Some(flag) = chain_params.exceptions.get(&hash) {
        return *flag;
    }

    // From Bitcoin Core:
    // BIP16 didn't become active until Apr 1 2012 (on mainnet, and
    // retroactively applied to testnet)
    // However, only one historical block violated the P2SH rules (on both
    // mainnet and testnet).
    // Similarly, only one historical block violated the TAPROOT rules on
    // mainnet.
    // For simplicity, always leave P2SH+WITNESS+TAPROOT on except for the two
    // violating blocks.
    let mut flags = bitcoinconsensus::VERIFY_P2SH | bitcoinconsensus::VERIFY_WITNESS;

    if height >= chain_params.params.bip65_height {
        flags |= bitcoinconsensus::VERIFY_CHECKLOCKTIMEVERIFY;
    }
    if height >= chain_params.params.bip66_height {
        flags |= bitcoinconsensus::VERIFY_DERSIG;
    }
    if height >= chain_params.csv_activation_height {
        flags |= bitcoinconsensus::VERIFY_CHECKSEQUENCEVERIFY;
    }
    if height >= chain_params.segwit_activation_height {
        flags |= bitcoinconsensus::VERIFY_NULLDUMMY;
    }
    flags
}
```

The flags cover the following consensus rules added to Bitcoin over time:

- P2SH ([BIP 16](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki)): Activated at height 173,805
- Enforce strict DER signatures ([BIP 66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki)): Activated at height 363,725
- CHECKLOCKTIMEVERIFY ([BIP 65](https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki)): Activated at height 388,381
- CHECKSEQUENCEVERIFY ([BIP 112](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki)): Activated at height 419,328
- Segregated Witness ([BIP 141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)) and Null Dummy ([BIP 147](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki)): Activated at height 481,824

### Verify Block Transactions

Now, the `Consensus::verify_block_transactions` function has this body, which in turn calls `Consensus::verify_transaction`:

Filename: pruned_utreexo/consensus.rs

```rust
# // Path: floresta-chain/src/pruned_utreexo/consensus.rs
#
// Omitted: impl Consensus {

/// Verify if all transactions in a block are valid. Here we check the following:
/// - The block must contain at least one transaction, and this transaction must be coinbase
/// - The first transaction in the block must be coinbase
/// - The coinbase transaction must have the correct value (subsidy + fees)
/// - The block must not create more coins than allowed
/// - All transactions must be valid:
///     - The transaction must not be coinbase (already checked)
///     - The transaction must not have duplicate inputs
///     - The transaction must not spend more coins than it claims in the inputs
///     - The transaction must have valid scripts
#[allow(unused)]
pub fn verify_block_transactions(
    height: u32,
    mut utxos: HashMap<OutPoint, UtxoData>,
    transactions: &[Transaction],
    subsidy: u64,
    verify_script: bool,
    flags: c_uint,
) -> Result<(), BlockchainError> {
    // Blocks must contain at least one transaction (i.e. the coinbase)
    if transactions.is_empty() {
        return Err(BlockValidationErrors::EmptyBlock)?;
    }

    // Total block fees that the miner can claim in the coinbase
    let mut fee = 0;

    for (n, transaction) in transactions.iter().enumerate() {
        if n == 0 {
            if !transaction.is_coinbase() {
                return Err(BlockValidationErrors::FirstTxIsNotCoinbase)?;
            }
            // Check coinbase input and output script limits
            Self::verify_coinbase(transaction)?;
            // Skip next checks: coinbase input is exempt, coinbase reward checked later
            continue;
        }

        // Actually verify the transaction
        let (in_value, out_value) =
            Self::verify_transaction(transaction, &mut utxos, height, verify_script, flags)?;

        // Fee is the difference between inputs and outputs
        fee += in_value - out_value;
    }

    // Check coinbase output values to ensure the miner isn't producing excess coins
    let allowed_reward = fee + subsidy;
    let coinbase_total: u64 = transactions[0]
        .output
        .iter()
        .map(|out| out.value.to_sat())
        .sum();

    if coinbase_total > allowed_reward {
        return Err(BlockValidationErrors::BadCoinbaseOutValue)?;
    }

    Ok(())
}

/// Verifies a single transaction. This function checks the following:
///     - The transaction doesn't spend more coins than it claims in the inputs
///     - The transaction doesn't create more coins than allowed
///     - The transaction has valid scripts
///     - The transaction doesn't have duplicate inputs (implicitly checked by the hashmap)
fn verify_transaction(
    transaction: &Transaction,
    utxos: &mut HashMap<OutPoint, UtxoData>,
    height: u32,
    _verify_script: bool,
    _flags: c_uint,
) -> Result<(u64, u64), BlockchainError> {
    let txid = || transaction.compute_txid();

    let out_value: u64 = transaction
        .output
        .iter()
        .map(|out| out.value.to_sat())
        .sum();

    let mut in_value = 0;
    for input in transaction.input.iter() {
        let utxo = Self::get_utxo(input, utxos, txid)?;
        let txout = &utxo.txout;

        // A coinbase output created at height n can only be spent at height >= n + 100
        if utxo.is_coinbase && (height < utxo.creation_height + 100) {
            return Err(tx_err!(txid, CoinbaseNotMatured))?;
        }

        // Check script sizes (spent txo pubkey, and current tx scriptsig and TODO witness)
        Self::validate_script_size(&txout.script_pubkey, txid)?;
        Self::validate_script_size(&input.script_sig, txid)?;
        // TODO check also witness script size

        in_value += txout.value.to_sat();
    }

    // Value in should be greater or equal to value out. Otherwise, inflation.
    if out_value > in_value {
        return Err(tx_err!(txid, NotEnoughMoney))?;
    }
    // Sanity check
    if out_value > 21_000_000 * COIN_VALUE {
        return Err(BlockValidationErrors::TooManyCoins)?;
    }

    // Verify the tx script
    #[cfg(feature = "bitcoinconsensus")]
    if _verify_script {
        transaction
            .verify_with_flags(
                |outpoint| utxos.remove(outpoint).map(|utxo| utxo.txout),
                _flags,
            )
            .map_err(|e| tx_err!(txid, ScriptValidationError, e.to_string()))?;
    };

    Ok((in_value, out_value))
}
```

In general the function behavior is well explained in the comments. Something to note is that we need the `bitcoinconsensus` feature set in order to use `verify_with_flags`, to verify the transaction scripts. If it's not set we won't perform script validation, so `bitcoinconsensus` should probably be mandatory, not just opt-in.

We also don't validate if `verify_script` is false, but this is because the `Assume-Valid` process has already assessed the scripts as valid.

<div class="warning">

Note that these consensus checks are far from complete. More checks will be added in the short term, but once `libbitcoinkernel` bindings are ready this function will instead use them.

</div>
