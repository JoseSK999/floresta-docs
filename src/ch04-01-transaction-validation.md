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
        # return Err(BlockchainError::BlockValidation(
            # BlockValidationErrors::BadMerkleRoot,
        # ));
    # }
    #
    # if height >= self.chain_params().params.bip34_height
        # && self.get_bip34_height(block) != Some(height)
    # {
        # return Err(BlockchainError::BlockValidation(
            # BlockValidationErrors::BadBip34,
        # ));
    # }
    #
    # if !block.check_witness_commitment() {
        # return Err(BlockchainError::BlockValidation(
            # BlockValidationErrors::BadWitnessCommitment,
        # ));
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
    // Blocks must contain at least one transaction
    if transactions.is_empty() {
        return Err(BlockValidationErrors::EmptyBlock.into());
    }
    let mut fee = 0;
    let mut wu: u64 = 0;
    // Skip the coinbase tx
    for (n, transaction) in transactions.iter().enumerate() {
        // We don't need to verify the coinbase inputs, as it spends newly generated coins
        if transaction.is_coinbase() && n == 0 {
            Self::verify_coinbase(transaction.clone(), n as u16).map_err(|err| {
                TransactionError {
                    txid: transaction.compute_txid(),
                    error: err,
                }
            });
            continue;
        }
        // Amount of all outputs
        let mut output_value = 0;
        for output in transaction.output.iter() {
            Self::get_out_value(output, &mut output_value).map_err(|err| TransactionError {
                txid: transaction.compute_txid(),
                error: err,
            });
            Self::validate_script_size(&output.script_pubkey).map_err(|err| TransactionError {
                txid: transaction.compute_txid(),
                error: err,
            });
        }
        // Amount of all inputs
        let mut in_value = 0;
        for input in transaction.input.iter() {
            Self::consume_utxos(input, &mut utxos, &mut in_value).map_err(|err| {
                TransactionError {
                    txid: transaction.compute_txid(),
                    error: err,
                }
            });
            Self::validate_script_size(&input.script_sig).map_err(|err| TransactionError {
                txid: transaction.compute_txid(),
                error: err,
            });
        }
        // Value in should be greater or equal to value out. Otherwise, inflation.
        if output_value > in_value {
            return Err(TransactionError {
                txid: transaction.compute_txid(),
                error: BlockValidationErrors::NotEnoughMoney,
            }
            .into());
        }
        if output_value > 21_000_000 * 100_000_000 {
            return Err(BlockValidationErrors::TooManyCoins.into());
        }
        // Fee is the difference between inputs and outputs
        fee += in_value - output_value;
        // Verify the tx script
        #[cfg(feature = "bitcoinconsensus")]
        if verify_script {
            transaction
                .verify_with_flags(|outpoint| utxos.remove(outpoint), flags)
                .map_err(|err| TransactionError {
                    txid: transaction.compute_txid(),
                    error: BlockValidationErrors::ScriptValidationError(err.to_string()),
                });
        };

        //checks vbytes validation
        //After all the checks, we sum the transaction weight to the block weight
        wu += transaction.weight().to_wu();
    }
    //checks if the block weight is fine.
    if wu > 4_000_000 {
        return Err(BlockValidationErrors::BlockTooBig.into());
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
