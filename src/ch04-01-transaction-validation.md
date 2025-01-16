## Transaction Validation

Let's now dive into `Consensus::verify_block_transactions`, to see how we verify the transactions in a block. As we saw in the [Block Validation](ch03-04-block-validation.md) section from last chapter, this function takes the height, the UTXOs to spend, the spending transactions, the current subsidy, the `verify_script` boolean (which was only true when we are not in the `Assume-Valid` range) and the validation flags.

```rust
# // Path: floresta-chain/src/pruned_utreexo/chain_state.rs
#
pub fn validate_block_no_acc(
    &self,
    block: &Block,
    height: u32,
    inputs: HashMap<OutPoint, TxOut>,
) -> Result<(), BlockchainError> {
    # if !block.check_merkle_root() {
        # return Err(BlockValidationErrors::BadMerkleRoot.into());
    # }
    #
    # let bip34_height = self.chain_params().params.bip34_height;
    # // If bip34 is active, check that the encoded block height is correct
    # if height >= bip34_height && self.get_bip34_height(block) != Some(height) {
        # return Err(BlockValidationErrors::BadBip34.into());
    # }
    #
    # if !block.check_witness_commitment() {
        # return Err(BlockValidationErrors::BadWitnessCommitment.into());
    # }
    #
    # if block.weight().to_wu() > 4_000_000 {
        # return Err(BlockValidationErrors::BlockTooBig.into());
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

Now, the `Consensus::verify_block_transactions` function has this body:

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
pub fn verify_block_transactions(
    height: u32,
    mut utxos: HashMap<OutPoint, TxOut>,
    transactions: &[Transaction],
    subsidy: u64,
    verify_script: bool,
    flags: c_uint,
) -> Result<(), BlockchainError> {
    // Blocks must contain at least one transaction (i.e. the coinbase)
    if transactions.is_empty() {
        return Err(BlockValidationErrors::EmptyBlock.into());
    }

    // Total block fees that the miner can claim in the coinbase
    let mut fee = 0;

    for (n, transaction) in transactions.iter().enumerate() {
        if n == 0 {
            if !transaction.is_coinbase() {
                return Err(BlockValidationErrors::FirstTxIsNotCoinbase.into());
            }

            Self::verify_coinbase(transaction).map_err(|error| TransactionError {
                txid: transaction.compute_txid(),
                error,
            })?;
            // Skip the rest of checks for the coinbase transaction
            continue;
        }

        // Sum tx output amounts, check their locking script sizes (scriptpubkey)
        let mut out_value = 0;
        for output in transaction.output.iter() {
            out_value += output.value.to_sat();

            Self::validate_script_size(&output.script_pubkey).map_err(|error| {
                TransactionError {
                    txid: transaction.compute_txid(),
                    error,
                }
            })?;
        }

        // Sum tx input amounts, check their unlocking script sizes (scriptsig and TODO witness)
        let mut in_value = 0;
        for input in transaction.input.iter() {
            let txo = Self::get_utxo(input, &utxos).map_err(|error| TransactionError {
                txid: transaction.compute_txid(),
                error,
            })?;

            in_value += txo.value.to_sat();

            Self::validate_script_size(&input.script_sig).map_err(|error| {
                TransactionError {
                    txid: transaction.compute_txid(),
                    error,
                }
            })?;
            // TODO check also witness script size
        }

        // Value in should be greater or equal to value out. Otherwise, inflation.
        if out_value > in_value {
            return Err(TransactionError {
                txid: transaction.compute_txid(),
                error: BlockValidationErrors::NotEnoughMoney,
            }
            .into());
        }
        // Sanity check
        if out_value > 21_000_000 * COIN_VALUE {
            return Err(BlockValidationErrors::TooManyCoins.into());
        }

        // Fee is the difference between inputs and outputs
        fee += in_value - out_value;

        // Verify the tx script
        #[cfg(feature = "bitcoinconsensus")]
        if verify_script {
            transaction
                .verify_with_flags(|outpoint| utxos.remove(outpoint), flags)
                .map_err(|err| TransactionError {
                    txid: transaction.compute_txid(),
                    error: BlockValidationErrors::ScriptValidationError(err.to_string()),
                })?;
        };
    }

    // Checks if the miner isn't trying to create inflation
    if fee + subsidy
        < transactions[0]
            .output
            .iter()
            .fold(0, |acc, out| acc + out.value.to_sat())
    {
        return Err(BlockValidationErrors::BadCoinbaseOutValue.into());
    }

    Ok(())
}
```

In general the function behavior is well explained in the comments. Something to note is that we need the `bitcoinconsensus` feature set in order to use `verify_with_flags`, to verify the transaction scripts. If it's not set we won't perform script validation, so `bitcoinconsensus` should probably be mandatory, not just opt-in.

We also don't validate if `verify_script` is false, but this is because the `Assume-Valid` process has already assessed the scripts as valid.

<div class="warning">

Note that these consensus checks are far from complete. More checks will be added in the short term, but once `libbitcoinkernel` bindings are ready this function will instead use them.

</div>
