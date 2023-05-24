// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    pruner::{
        db_sub_pruner::DBSubPruner, pruner_utils::get_and_maybe_update_ledger_subpruner_progress,
    },
    schema::db_metadata::{DbMetadataKey, DbMetadataSchema, DbMetadataValue},
    EventStore,
};
use anyhow::{ensure, Result};
use aptos_schemadb::{SchemaBatch, DB};
use aptos_types::transaction::{AtomicVersion, Version};
use std::sync::{atomic::Ordering, Arc};

#[derive(Debug)]
pub struct EventStorePruner {
    event_store: Arc<EventStore>,
    event_db: Arc<DB>,
    progress: AtomicVersion,
}

impl DBSubPruner for EventStorePruner {
    fn prune(&self, progress: Version, target_version: Version) -> Result<()> {
        let stored_progress = self.progress.load(Ordering::SeqCst);
        ensure!(
            progress == stored_progress || stored_progress == 0,
            "Progress for Event doesn't match, {progress} vs {stored_progress}.",
        );

        let batch = SchemaBatch::new();
        self.event_store
            .prune_events(progress, target_version, &batch)?;
        batch.put::<DbMetadataSchema>(
            &DbMetadataKey::EventPrunerProgress,
            &DbMetadataValue::Version(target_version),
        )?;
        self.event_db.write_schemas(batch)?;

        self.progress.store(target_version, Ordering::SeqCst);

        Ok(())
    }

    fn progress(&self) -> Version {
        self.progress.load(Ordering::SeqCst)
    }
}

impl EventStorePruner {
    pub(in crate::pruner) fn new(
        event_store: Arc<EventStore>,
        event_db: Arc<DB>,
        metadata_progress: Version,
    ) -> Result<Self> {
        let progress = get_and_maybe_update_ledger_subpruner_progress(
            &event_db,
            &DbMetadataKey::EventPrunerProgress,
            metadata_progress,
        )?;

        Ok(EventStorePruner {
            event_store,
            event_db,
            progress: AtomicVersion::new(progress),
        })
    }
}
