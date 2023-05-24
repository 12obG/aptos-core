// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::schema::{
    db_metadata::{DbMetadataKey, DbMetadataSchema, DbMetadataValue},
    version_data::VersionDataSchema,
};
use anyhow::{ensure, Result};
use aptos_schemadb::{ReadOptions, SchemaBatch, DB};
use aptos_types::transaction::{AtomicVersion, Version};
use std::sync::{atomic::Ordering, Arc};

#[derive(Debug)]
pub struct LedgerMetadataPruner {
    ledger_metadata_db: Arc<DB>,
    progress: AtomicVersion,
}

impl LedgerMetadataPruner {
    pub(in crate::pruner) fn new(ledger_metadata_db: Arc<DB>) -> Result<Self> {
        let progress = if let Some(v) =
            ledger_metadata_db.get::<DbMetadataSchema>(&DbMetadataKey::LedgerPrunerProgress)?
        {
            v.expect_version()
        } else {
            let mut iter = ledger_metadata_db.iter::<VersionDataSchema>(ReadOptions::default())?;
            iter.seek_to_first();
            let version = match iter.next().transpose()? {
                Some((version, _)) => version,
                None => 0,
            };
            ledger_metadata_db.put::<DbMetadataSchema>(
                &DbMetadataKey::LedgerPrunerProgress,
                &DbMetadataValue::Version(version),
            )?;
            version
        };

        Ok(LedgerMetadataPruner {
            ledger_metadata_db,
            progress: AtomicVersion::new(progress),
        })
    }

    pub(in crate::pruner) fn prune(
        &self,
        progress: Version,
        target_version: Version,
    ) -> Result<()> {
        let stored_progress = self.progress();
        ensure!(
            progress == stored_progress,
            "Progress for LedgerMetadata doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        for version in progress..target_version {
            batch.delete::<VersionDataSchema>(&version)?;
        }
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::LedgerPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.ledger_metadata_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    pub(in crate::pruner) fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}
