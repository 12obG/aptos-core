// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    pruner::{
        db_sub_pruner::DBSubPruner, pruner_utils::get_and_maybe_update_ledger_subpruner_progress,
    },
    schema::db_metadata::{DbMetadataKey, DbMetadataSchema, DbMetadataValue},
    TransactionStore,
};
use anyhow::{ensure, Result};
use aptos_schemadb::{SchemaBatch, DB};
use aptos_types::transaction::{AtomicVersion, Transaction, Version};
use std::sync::{atomic::Ordering, Arc};

#[derive(Debug)]
pub struct TransactionPruner {
    transaction_store: Arc<TransactionStore>,
    transaction_db: Arc<DB>,
    progress: AtomicVersion,
}

impl DBSubPruner for TransactionPruner {
    fn prune(&self, progress: Version, target_version: Version) -> Result<()> {
        let stored_progress = self.progress();
        ensure!(
            progress == stored_progress,
            "Progress for Transaction doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        let candidate_transactions =
            self.get_pruning_candidate_transactions(progress, target_version)?;
        self.transaction_store
            .prune_transaction_by_hash(&candidate_transactions, &batch)?;
        self.transaction_store
            .prune_transaction_by_account(&candidate_transactions, &batch)?;
        self.transaction_store
            .prune_transaction_schema(progress, target_version, &batch)?;
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::TransactionPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.transaction_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}

impl TransactionPruner {
    pub(in crate::pruner) fn new(
        transaction_store: Arc<TransactionStore>,
        transaction_db: Arc<DB>,
        metadata_progress: Version,
    ) -> Result<Self> {
        let progress = get_and_maybe_update_ledger_subpruner_progress(
            &transaction_db,
            &DbMetadataKey::TransactionPrunerProgress,
            metadata_progress,
        )?;

        Ok(TransactionPruner {
            transaction_store,
            transaction_db,
            progress: AtomicVersion::new(progress),
        })
    }

    fn get_pruning_candidate_transactions(
        &self,
        start: Version,
        end: Version,
    ) -> anyhow::Result<Vec<Transaction>> {
        self.transaction_store
            .get_transaction_iter(start, (end - start) as usize)?
            .collect()
    }
}
