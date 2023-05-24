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
use aptos_types::transaction::{AtomicVersion, Version};
use std::sync::{atomic::Ordering, Arc};

#[derive(Debug)]
pub struct TransactionAccumulatorPruner {
    transaction_store: Arc<TransactionStore>,
    transaction_accumulator_db: Arc<DB>,
    progress: AtomicVersion,
}

impl DBSubPruner for TransactionAccumulatorPruner {
    fn prune(&self, progress: Version, target_version: Version) -> Result<()> {
        let stored_progress = self.progress.load(Ordering::SeqCst);
        ensure!(
            progress == stored_progress || stored_progress == 0,
            "Progress for TransactionAccumulator doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        self.transaction_store
            .prune_transaction_accumulator(progress, target_version, &batch)?;
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::TransactionAccumulatorPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.transaction_accumulator_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}

impl TransactionAccumulatorPruner {
    pub(in crate::pruner) fn new(
        transaction_store: Arc<TransactionStore>,
        transaction_accumulator_db: Arc<DB>,
        metadata_progress: Version,
    ) -> Result<Self> {
        let progress = get_and_maybe_update_ledger_subpruner_progress(
            &transaction_accumulator_db,
            &DbMetadataKey::TransactionAccumulatorPrunerProgress,
            metadata_progress,
        )?;

        Ok(TransactionAccumulatorPruner {
            transaction_store,
            transaction_accumulator_db,
            progress: AtomicVersion::new(progress),
        })
    }
}
