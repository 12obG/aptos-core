// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use aptos_types::transaction::Version;

/// Defines the trait for sub-pruner of a parent DB pruner
pub trait DBSubPruner {
    /// Performs the actual pruning, a target version is passed, which is the target the pruner
    /// tries to prune.
    fn prune(&self, min_readable_version: Version, target_version: Version) -> anyhow::Result<()>;

    // Returns the progress of the pruner.
    fn progress(&self) -> Version;

    // Catches up the progress to `target_version`.
    fn catch_up(&self, target_version: Version) -> anyhow::Result<()> {
        self.prune(self.progress(), target_version)
    }
}
