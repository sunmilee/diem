// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use crate::{
    components::chunk_output::ChunkOutput, metrics::DIEM_EXECUTOR_ERRORS, process_write_set,
};
use anyhow::{anyhow, ensure, Result};
use diem_crypto::{
    hash::{CryptoHash, EventAccumulatorHasher, TransactionAccumulatorHasher},
    HashValue,
};
use diem_logger::error;
use diem_types::{
    account_address::{AccountAddress, HashAccountAddress},
    account_state::AccountState,
    account_state_blob::AccountStateBlob,
    epoch_state::EpochState,
    nibble::nibble_path::NibblePath,
    on_chain_config,
    proof::accumulator::InMemoryAccumulator,
    transaction::{
        Transaction, TransactionInfo, TransactionInfoTrait, TransactionOutput, TransactionStatus,
    },
};
use executor_types::{ExecutedChunk, ExecutedTrees, ProofReader, TransactionData};
use rayon::prelude::*;
use scratchpad::SparseMerkleTree;
use std::{collections::HashMap, convert::TryFrom, sync::Arc};
use storage_interface::state_view::StateCache;

pub struct ApplyChunkOutput;

impl ApplyChunkOutput {
    pub fn apply(
        chunk_output: ChunkOutput,
        base_accumulator: &Arc<InMemoryAccumulator<TransactionAccumulatorHasher>>,
    ) -> Result<(ExecutedChunk, Vec<Transaction>, Vec<Transaction>)> {
        let ChunkOutput {
            state_cache,
            transactions,
            transaction_outputs,
        } = chunk_output;

        // Separate transactions with different VM statuses.
        let (new_epoch, to_keep, to_discard, to_retry) =
            Self::sort_transactions(transactions, transaction_outputs)?;

        // Apply the write set, get the latest state.
        let (account_blobs, roots_with_node_hashes, result_state, next_epoch_state) =
            Self::apply_write_set(state_cache, new_epoch, &to_keep)?;

        // Calculate TransactionData and TransactionInfo, i.e. the ledger history diff.
        let (to_commit, transaction_info_hashes) =
            Self::assemble_ledger_diff(to_keep, account_blobs, roots_with_node_hashes);

        Ok((
            ExecutedChunk {
                to_commit,
                result_view: ExecutedTrees::new_copy(
                    result_state,
                    Arc::new(base_accumulator.append(&transaction_info_hashes)),
                ),
                next_epoch_state,
                ledger_info: None,
            },
            to_discard,
            to_retry,
        ))
    }

    fn sort_transactions(
        mut transactions: Vec<Transaction>,
        mut transaction_outputs: Vec<TransactionOutput>,
    ) -> Result<(
        bool,
        Vec<(Transaction, TransactionOutput)>,
        Vec<Transaction>,
        Vec<Transaction>,
    )> {
        // See if there's a new epoch started
        let new_epoch_event_key = on_chain_config::new_epoch_event_key();
        let new_epoch_marker = transaction_outputs
            .iter()
            .position(|o| {
                o.events()
                    .iter()
                    .any(|event| *event.key() == new_epoch_event_key)
            })
            // Off by one for exclusive index.
            .map(|idx| idx + 1);

        // Transactions after the epoch ending are all to be retried.
        let to_retry = if let Some(pos) = new_epoch_marker {
            transaction_outputs.drain(pos..);
            transactions.drain(pos..).collect()
        } else {
            vec![]
        };

        // Separate transactions with the Keep status out.
        let (to_keep, to_discard) =
            itertools::zip_eq(transactions.into_iter(), transaction_outputs.into_iter())
                .partition::<Vec<(Transaction, TransactionOutput)>, _>(|(_, o)| {
                    matches!(o.status(), TransactionStatus::Keep(_))
                });

        // Sanity check transactions with the Discard status:
        let to_discard = to_discard
            .into_iter()
            .map(|(t, o)| {
                // In case a new status other than Retry, Keep and Discard is added:
                if !matches!(o.status(), TransactionStatus::Discard(_)) {
                    error!("Status other than Retry, Keep or Discard; Transaction discarded.");
                }
                // VM shouldn't have output anything for discarded transactions, log if it did.
                if !o.write_set().is_empty() || !o.events().is_empty() {
                    error!(
                        "Discarded transaction has non-empty write set or events. \
                     Transaction: {:?}. Status: {:?}.",
                        t,
                        o.status(),
                    );
                    DIEM_EXECUTOR_ERRORS.inc();
                }
                Ok(t)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((new_epoch_marker.is_some(), to_keep, to_discard, to_retry))
    }

    fn apply_write_set(
        state_cache: StateCache,
        new_epoch: bool,
        to_keep: &[(Transaction, TransactionOutput)],
    ) -> Result<(
        Vec<HashMap<AccountAddress, AccountStateBlob>>,
        Vec<(HashValue, HashMap<NibblePath, HashValue>)>,
        SparseMerkleTree<AccountStateBlob>,
        Option<EpochState>,
    )> {
        let StateCache {
            frozen_base,
            mut accounts,
            proofs,
        } = state_cache;

        // Apply write sets to account states in the AccountCache, resulting in new account states.
        let account_states = to_keep
            .iter()
            .map(|(t, o)| process_write_set(t, &mut accounts, o.write_set().clone()))
            .collect::<Result<Vec<_>>>()?;
        let account_blobs = account_states
            .par_iter()
            .with_min_len(100)
            .map(|account_to_state| {
                account_to_state
                    .iter()
                    .map(|(addr, state)| Ok((*addr, AccountStateBlob::try_from(state)?)))
                    .collect::<Result<HashMap<_, _>>>()
            })
            .collect::<Result<Vec<_>>>()?;

        // Apply new account states to the base state tree, resulting in updated state tree.
        let (roots_with_node_hashes, result_state) = frozen_base
            .serial_update(
                Self::account_blobs_to_smt_updates(&account_blobs),
                &ProofReader::new(proofs),
            )
            .map_err(|e| anyhow!("Failed to update state tree. err: {:?}", e))?;
        // Release ASAP the ref to the base SMT to allow old in-mem nodes to be dropped,
        // now that we don't require access to them.
        let result_state = result_state.unfreeze();

        // Get the updated validator set from updated account state.
        let next_epoch_state = if new_epoch {
            Some(Self::parse_validator_set(&accounts)?)
        } else {
            None
        };

        Ok((
            account_blobs,
            roots_with_node_hashes,
            result_state,
            next_epoch_state,
        ))
    }

    fn account_blobs_to_smt_updates(
        account_blobs: &[HashMap<AccountAddress, AccountStateBlob>],
    ) -> Vec<Vec<(HashValue, &AccountStateBlob)>> {
        account_blobs
            .iter()
            .map(|m| {
                m.iter()
                    .map(|(account, blob)| (HashAccountAddress::hash(account), blob))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn parse_validator_set(accounts: &HashMap<AccountAddress, AccountState>) -> Result<EpochState> {
        let validator_set = accounts
            .get(&on_chain_config::config_address())
            .map(|state| {
                state
                    .get_validator_set()?
                    .ok_or_else(|| anyhow!("ValidatorSet does not exist"))
            })
            .ok_or_else(|| anyhow!("ValidatorSet account does not exist"))??;
        let configuration = accounts
            .get(&on_chain_config::config_address())
            .map(|state| {
                state
                    .get_configuration_resource()?
                    .ok_or_else(|| anyhow!("Configuration does not exist"))
            })
            .ok_or_else(|| anyhow!("Association account does not exist"))??;

        Ok(EpochState {
            epoch: configuration.epoch(),
            verifier: (&validator_set).into(),
        })
    }

    fn assemble_ledger_diff(
        to_keep: Vec<(Transaction, TransactionOutput)>,
        account_blobs: Vec<HashMap<AccountAddress, AccountStateBlob>>,
        roots_with_node_hashes: Vec<(HashValue, HashMap<NibblePath, HashValue>)>,
    ) -> (Vec<(Transaction, TransactionData)>, Vec<HashValue>) {
        let mut to_commit = vec![];
        let mut txn_info_hashes = vec![];
        for ((txn, txn_output), ((state_tree_hash, new_node_hashes), blobs)) in itertools::zip_eq(
            to_keep,
            itertools::zip_eq(roots_with_node_hashes, account_blobs),
        ) {
            let event_tree = {
                let event_hashes: Vec<_> =
                    txn_output.events().iter().map(CryptoHash::hash).collect();
                InMemoryAccumulator::<EventAccumulatorHasher>::from_leaves(&event_hashes)
            };

            let txn_info = match txn_output.status() {
                TransactionStatus::Keep(status) => TransactionInfo::new(
                    txn.hash(),
                    state_tree_hash,
                    event_tree.root_hash(),
                    txn_output.gas_used(),
                    status.clone(),
                ),
                _ => unreachable!("Transaction sorted by status already."),
            };

            let txn_info_hash = txn_info.hash();
            txn_info_hashes.push(txn_info_hash);
            to_commit.push((
                txn,
                TransactionData::new(
                    blobs,
                    new_node_hashes,
                    txn_output.write_set().clone(),
                    txn_output.events().to_vec(),
                    txn_output.status().clone(),
                    state_tree_hash,
                    Arc::new(event_tree),
                    txn_output.gas_used(),
                    Some(txn_info_hash),
                ),
            ))
        }
        (to_commit, txn_info_hashes)
    }
}

pub fn ensure_no_discard(to_discard: Vec<Transaction>) -> Result<()> {
    ensure!(to_discard.is_empty(), "Syncing discarded transactions");
    Ok(())
}

pub fn ensure_no_retry(to_retry: Vec<Transaction>) -> Result<()> {
    ensure!(to_retry.is_empty(), "Chunk crosses epoch boundary.",);
    Ok(())
}
