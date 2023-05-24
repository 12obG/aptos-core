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
pub struct TransactionInfoPruner {
    transaction_store: Arc<TransactionStore>,
    transaction_info_db: Arc<DB>,
    progress: AtomicVersion,
}

impl DBSubPruner for TransactionInfoPruner {
    fn prune(&self, progress: Version, target_version: Version) -> anyhow::Result<()> {
        let stored_progress = self.progress.load(Ordering::SeqCst);
        ensure!(
            progress == stored_progress || stored_progress == 0,
            "Progress for TransactionInfo doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        self.transaction_store
            .prune_transaction_info_schema(progress, target_version, &batch)?;
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::TransactionInfoPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.transaction_info_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}

impl TransactionInfoPruner {
    pub(in crate::pruner) fn new(
        transaction_store: Arc<TransactionStore>,
        transaction_info_db: Arc<DB>,
        metadata_progress: Version,
    ) -> Result<Self> {
        let progress = get_and_maybe_update_ledger_subpruner_progress(
            &transaction_info_db,
            &DbMetadataKey::TransactionInfoPrunerProgress,
            metadata_progress,
        )?;

        Ok(TransactionInfoPruner {
            transaction_store,
            transaction_info_db,
            progress: AtomicVersion::new(progress),
        })
    }
}
