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
pub struct WriteSetPruner {
    transaction_store: Arc<TransactionStore>,
    write_set_db: Arc<DB>,
    progress: AtomicVersion,
}

impl DBSubPruner for WriteSetPruner {
    fn prune(&self, progress: Version, target_version: Version) -> anyhow::Result<()> {
        let stored_progress = self.progress.load(Ordering::SeqCst);
        ensure!(
            progress == stored_progress,
            "Progress for WriteSet doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        self.transaction_store
            .prune_write_set(progress, target_version, &batch)?;
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::WriteSetPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.write_set_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}

impl WriteSetPruner {
    pub(in crate::pruner) fn new(
        transaction_store: Arc<TransactionStore>,
        write_set_db: Arc<DB>,
        metadata_progress: Version,
    ) -> Result<Self> {
        let progress = get_and_maybe_update_ledger_subpruner_progress(
            &write_set_db,
            &DbMetadataKey::WriteSetPrunerProgress,
            metadata_progress,
        )?;

        Ok(WriteSetPruner {
            transaction_store,
            write_set_db,
            progress: AtomicVersion::new(progress),
        })
    }
}
