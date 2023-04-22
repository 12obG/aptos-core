// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::config::{
    config_optimizer::ConfigOptimizer, config_sanitizer::ConfigSanitizer,
    node_config_loader::NodeType, Error, NodeConfig,
};
use aptos_types::chain_id::ChainId;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

// The maximum message size per state sync message
const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024; /* 4 MiB */

// The maximum chunk sizes for data client requests and response
const MAX_EPOCH_CHUNK_SIZE: u64 = 200;
const MAX_STATE_CHUNK_SIZE: u64 = 4000;
const MAX_TRANSACTION_CHUNK_SIZE: u64 = 2000;
const MAX_TRANSACTION_OUTPUT_CHUNK_SIZE: u64 = 1000;

// The maximum number of concurrent requests to send
const MAX_CONCURRENT_REQUESTS: u64 = 6;
const MAX_CONCURRENT_STATE_REQUESTS: u64 = 6;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StateSyncConfig {
    pub data_streaming_service: DataStreamingServiceConfig,
    pub aptos_data_client: AptosDataClientConfig,
    pub state_sync_driver: StateSyncDriverConfig,
    pub storage_service: StorageServiceConfig,
}

/// The bootstrapping mode determines how the node will bootstrap to the latest
/// blockchain state, e.g., directly download the latest states.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum BootstrappingMode {
    ApplyTransactionOutputsFromGenesis, // Applies transaction outputs (starting at genesis)
    DownloadLatestStates, // Downloads the state keys and values (at the latest version)
    ExecuteTransactionsFromGenesis, // Executes transactions (starting at genesis)
    ExecuteOrApplyFromGenesis, // Executes transactions or applies outputs from genesis (whichever is faster)
}

impl BootstrappingMode {
    pub fn to_label(&self) -> &'static str {
        match self {
            BootstrappingMode::ApplyTransactionOutputsFromGenesis => {
                "apply_transaction_outputs_from_genesis"
            },
            BootstrappingMode::DownloadLatestStates => "download_latest_states",
            BootstrappingMode::ExecuteTransactionsFromGenesis => {
                "execute_transactions_from_genesis"
            },
            BootstrappingMode::ExecuteOrApplyFromGenesis => "execute_or_apply_from_genesis",
        }
    }
}

/// The continuous syncing mode determines how the node will stay up-to-date
/// once it has bootstrapped and the blockchain continues to grow, e.g.,
/// continuously executing all transactions.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ContinuousSyncingMode {
    ApplyTransactionOutputs, // Applies transaction outputs to stay up-to-date
    ExecuteTransactions,     // Executes transactions to stay up-to-date
    ExecuteTransactionsOrApplyOutputs, // Executes transactions or applies outputs to stay up-to-date (whichever is faster)
}

impl ContinuousSyncingMode {
    pub fn to_label(&self) -> &'static str {
        match self {
            ContinuousSyncingMode::ApplyTransactionOutputs => "apply_transaction_outputs",
            ContinuousSyncingMode::ExecuteTransactions => "execute_transactions",
            ContinuousSyncingMode::ExecuteTransactionsOrApplyOutputs => {
                "execute_transactions_or_apply_outputs"
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StateSyncDriverConfig {
    pub bootstrapping_mode: BootstrappingMode, // The mode by which to bootstrap
    pub commit_notification_timeout_ms: u64, // The max time taken to process a commit notification
    pub continuous_syncing_mode: ContinuousSyncingMode, // The mode by which to sync after bootstrapping
    pub enable_auto_bootstrapping: bool, // Enable auto-bootstrapping if no peers are found after `max_connection_deadline_secs`
    pub fallback_to_output_syncing_secs: u64, // The duration to fallback to output syncing after an execution failure
    pub progress_check_interval_ms: u64, // The interval (ms) at which to check state sync progress
    pub max_connection_deadline_secs: u64, // The max time (secs) to wait for connections from peers before auto-bootstrapping
    pub max_consecutive_stream_notifications: u64, // The max number of notifications to process per driver loop
    pub max_num_stream_timeouts: u64, // The max number of stream timeouts allowed before termination
    pub max_pending_data_chunks: u64, // The max number of data chunks pending execution or commit
    pub max_stream_wait_time_ms: u64, // The max time (ms) to wait for a data stream notification
    pub num_versions_to_skip_snapshot_sync: u64, // The version lag we'll tolerate before snapshot syncing
}

/// The default state sync driver config will be the one that gets (and keeps)
/// the node up-to-date as quickly and cheaply as possible.
impl Default for StateSyncDriverConfig {
    fn default() -> Self {
        Self {
            bootstrapping_mode: BootstrappingMode::ApplyTransactionOutputsFromGenesis,
            commit_notification_timeout_ms: 5000,
            continuous_syncing_mode: ContinuousSyncingMode::ApplyTransactionOutputs,
            enable_auto_bootstrapping: false,
            fallback_to_output_syncing_secs: 180, // 3 minutes
            progress_check_interval_ms: 100,
            max_connection_deadline_secs: 10,
            max_consecutive_stream_notifications: 10,
            max_num_stream_timeouts: 12,
            max_pending_data_chunks: 100,
            max_stream_wait_time_ms: 5000,
            num_versions_to_skip_snapshot_sync: 100_000_000, // At 5k TPS, this allows a node to fail for about 6 hours.
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageServiceConfig {
    pub max_concurrent_requests: u64, // Max num of concurrent storage server tasks
    pub max_epoch_chunk_size: u64,    // Max num of epoch ending ledger infos per chunk
    pub max_lru_cache_size: u64,      // Max num of items in the lru cache before eviction
    pub max_network_channel_size: u64, // Max num of pending network messages
    pub max_network_chunk_bytes: u64, // Max num of bytes to send per network message
    pub max_state_chunk_size: u64,    // Max num of state keys and values per chunk
    pub max_subscription_period_ms: u64, // Max period (ms) of pending subscription requests
    pub max_transaction_chunk_size: u64, // Max num of transactions per chunk
    pub max_transaction_output_chunk_size: u64, // Max num of transaction outputs per chunk
    pub storage_summary_refresh_interval_ms: u64, // The interval (ms) to refresh the storage summary
}

impl Default for StorageServiceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 4000,
            max_epoch_chunk_size: MAX_EPOCH_CHUNK_SIZE,
            max_lru_cache_size: 500, // At ~0.6MiB per chunk, this should take no more than 0.5GiB
            max_network_channel_size: 4000,
            max_network_chunk_bytes: MAX_MESSAGE_SIZE as u64,
            max_state_chunk_size: MAX_STATE_CHUNK_SIZE,
            max_subscription_period_ms: 5000,
            max_transaction_chunk_size: MAX_TRANSACTION_CHUNK_SIZE,
            max_transaction_output_chunk_size: MAX_TRANSACTION_OUTPUT_CHUNK_SIZE,
            storage_summary_refresh_interval_ms: 50,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DataStreamingServiceConfig {
    // The interval (milliseconds) at which to refresh the global data summary.
    pub global_summary_refresh_interval_ms: u64,

    // Maximum number of concurrent data client requests (per stream).
    pub max_concurrent_requests: u64,

    // Maximum number of concurrent data client requests (per stream) for state keys/values.
    pub max_concurrent_state_requests: u64,

    // Maximum channel sizes for each data stream listener. If messages are not
    // consumed, they will be dropped (oldest messages first). The remaining
    // messages will be retrieved using FIFO ordering.
    pub max_data_stream_channel_sizes: u64,

    // Maximum number of retries for a single client request before a data
    // stream will terminate.
    pub max_request_retry: u64,

    // Maximum number of notification ID to response context mappings held in
    // memory. Once the number grows beyond this value, garbage collection occurs.
    pub max_notification_id_mappings: u64,

    // The interval (milliseconds) at which to check the progress of each stream.
    pub progress_check_interval_ms: u64,
}

impl Default for DataStreamingServiceConfig {
    fn default() -> Self {
        Self {
            global_summary_refresh_interval_ms: 50,
            max_concurrent_requests: MAX_CONCURRENT_REQUESTS,
            max_concurrent_state_requests: MAX_CONCURRENT_REQUESTS,
            max_data_stream_channel_sizes: 300,
            max_request_retry: 5,
            max_notification_id_mappings: 300,
            progress_check_interval_ms: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AptosDataClientConfig {
    pub max_epoch_chunk_size: u64, // Max num of epoch ending ledger infos per chunk
    pub max_num_in_flight_priority_polls: u64, // Max num of in-flight polls for priority peers
    pub max_num_in_flight_regular_polls: u64, // Max num of in-flight polls for regular peers
    pub max_num_output_reductions: u64, // The max num of output reductions before transactions are returned
    pub max_response_timeout_ms: u64, // Max timeout (in ms) when waiting for a response (after exponential increases)
    pub max_state_chunk_size: u64,    // Max num of state keys and values per chunk
    pub max_transaction_chunk_size: u64, // Max num of transactions per chunk
    pub max_transaction_output_chunk_size: u64, // Max num of transaction outputs per chunk
    pub response_timeout_ms: u64,     // First timeout (in ms) when waiting for a response
    pub subscription_timeout_ms: u64, // Timeout (in ms) when waiting for a subscription response
    pub summary_poll_interval_ms: u64, // Interval (in ms) between data summary polls
    pub use_compression: bool,        // Whether or not to request compression for incoming data
}

impl Default for AptosDataClientConfig {
    fn default() -> Self {
        Self {
            max_epoch_chunk_size: MAX_EPOCH_CHUNK_SIZE,
            max_num_in_flight_priority_polls: 10,
            max_num_in_flight_regular_polls: 10,
            max_num_output_reductions: 0,
            max_response_timeout_ms: 60000, // 60 seconds
            max_state_chunk_size: MAX_STATE_CHUNK_SIZE,
            max_transaction_chunk_size: MAX_TRANSACTION_CHUNK_SIZE,
            max_transaction_output_chunk_size: MAX_TRANSACTION_OUTPUT_CHUNK_SIZE,
            response_timeout_ms: 10000,    // 10 seconds
            subscription_timeout_ms: 5000, // 5 seconds
            summary_poll_interval_ms: 200,
            use_compression: true,
        }
    }
}

impl ConfigSanitizer for StateSyncConfig {
    /// Validate and process the state sync config according to the given node type and chain ID
    fn sanitize(
        _node_config: &mut NodeConfig,
        _node_type: NodeType,
        _chain_id: ChainId,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl ConfigOptimizer for StateSyncConfig {
    /// Optimize the state sync config according to the given node type and chain ID
    fn optimize(
        node_config: &mut NodeConfig,
        local_config_yaml: &Value,
        node_type: NodeType,
        chain_id: ChainId,
    ) -> Result<(), Error> {
        // Optimize the driver and data streaming service configs
        StateSyncDriverConfig::optimize(node_config, local_config_yaml, node_type, chain_id)?;
        DataStreamingServiceConfig::optimize(node_config, local_config_yaml, node_type, chain_id)?;

        Ok(())
    }
}

impl ConfigOptimizer for StateSyncDriverConfig {
    /// Optimize the driver config according to the given node type and chain ID
    fn optimize(
        node_config: &mut NodeConfig,
        local_config_yaml: &Value,
        _node_type: NodeType,
        chain_id: ChainId,
    ) -> Result<(), Error> {
        let state_sync_driver_config = &mut node_config.state_sync.state_sync_driver;
        let local_driver_config_yaml = &local_config_yaml["state_sync"]["state_sync_driver"];

        // Default to fast sync for all testnet nodes
        if chain_id.is_testnet() && local_driver_config_yaml["bootstrapping_mode"].is_null() {
            state_sync_driver_config.bootstrapping_mode = BootstrappingMode::DownloadLatestStates;
        }

        Ok(())
    }
}

impl ConfigOptimizer for DataStreamingServiceConfig {
    /// Optimize the data streaming service config according to the given node type and chain ID
    fn optimize(
        node_config: &mut NodeConfig,
        local_config_yaml: &Value,
        node_type: NodeType,
        _chain_id: ChainId,
    ) -> Result<(), Error> {
        let data_streaming_service_config = &mut node_config.state_sync.data_streaming_service;
        let local_stream_config_yaml = &local_config_yaml["state_sync"]["data_streaming_service"];

        // Double the aggression of the pre-fetcher for validators and VFNs
        if node_type.is_validator() || node_type.is_validator_fullnode() {
            // Double transaction prefetching
            if local_stream_config_yaml["max_concurrent_requests"].is_null() {
                data_streaming_service_config.max_concurrent_requests = MAX_CONCURRENT_REQUESTS * 2;
            }

            // Double state-value prefetching
            if local_stream_config_yaml["max_concurrent_state_requests"].is_null() {
                data_streaming_service_config.max_concurrent_state_requests =
                    MAX_CONCURRENT_STATE_REQUESTS * 2;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_bootstrapping_mode_testnet_vfn() {
        // Create a node config with execution mode enabled
        let mut node_config = create_execution_mode_config();

        // Create a local config YAML without any relevant fields
        let local_config_yaml = generate_local_config_yaml(false);

        // Verify that the bootstrapping mode is changed for a testnet VFN
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::ValidatorFullnode,
            ChainId::testnet(),
        )
        .unwrap();
        assert_eq!(
            node_config.state_sync.state_sync_driver.bootstrapping_mode,
            BootstrappingMode::DownloadLatestStates
        );
    }

    #[test]
    fn test_optimize_bootstrapping_mode_testnet_validator() {
        // Create a node config with execution mode enabled
        let mut node_config = create_execution_mode_config();

        // Create a local config YAML without any relevant fields
        let local_config_yaml = generate_local_config_yaml(false);

        // Verify that the bootstrapping mode is changed for a testnet validator
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::ValidatorFullnode,
            ChainId::testnet(),
        )
        .unwrap();
        assert_eq!(
            node_config.state_sync.state_sync_driver.bootstrapping_mode,
            BootstrappingMode::DownloadLatestStates
        );
    }

    #[test]
    fn test_optimize_bootstrapping_mode_mainnet_vfn() {
        // Create a node config with execution mode enabled
        let mut node_config = create_execution_mode_config();

        // Create a local config YAML without any relevant fields
        let local_config_yaml = generate_local_config_yaml(false);

        // Verify that the bootstrapping mode is not changed for a mainnet VFN
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::ValidatorFullnode,
            ChainId::mainnet(),
        )
        .unwrap();
        assert_eq!(
            node_config.state_sync.state_sync_driver.bootstrapping_mode,
            BootstrappingMode::ExecuteTransactionsFromGenesis
        );
    }

    #[test]
    fn test_optimize_bootstrapping_mode_no_override() {
        // Create a node config with execution mode enabled
        let mut node_config = create_execution_mode_config();

        // Create a local config YAML with the bootstrapping mode set to execution mode
        let local_config_yaml = generate_local_config_yaml(true);

        // Verify that the bootstrapping mode is not changed for a testnet VFN
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::ValidatorFullnode,
            ChainId::testnet(),
        )
        .unwrap();
        assert_eq!(
            node_config.state_sync.state_sync_driver.bootstrapping_mode,
            BootstrappingMode::ExecuteTransactionsFromGenesis
        );
    }

    #[test]
    fn test_optimize_prefetcher_mainnet_validator() {
        // Create a default node config
        let mut node_config = NodeConfig::default();

        // Create a local config YAML without any relevant fields
        let local_config_yaml = generate_local_config_yaml(false);

        // Verify that the prefetcher aggression has doubled
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::Validator,
            ChainId::mainnet(),
        )
        .unwrap();
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_requests,
            MAX_CONCURRENT_REQUESTS * 2
        );
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_state_requests,
            MAX_CONCURRENT_STATE_REQUESTS * 2
        );
    }

    #[test]
    fn test_optimize_prefetcher_testnet_pfn() {
        // Create a default node config
        let mut node_config = NodeConfig::default();

        // Create a local config YAML without any relevant fields
        let local_config_yaml = generate_local_config_yaml(false);

        // Verify that the prefetcher aggression has not changed
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::PublicFullnode,
            ChainId::testnet(),
        )
        .unwrap();
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_requests,
            MAX_CONCURRENT_REQUESTS
        );
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_state_requests,
            MAX_CONCURRENT_STATE_REQUESTS
        );
    }

    #[test]
    fn test_optimize_prefetcher_vfn_no_override() {
        // Create a node config where the state prefetcher is set to 100
        let mut node_config = NodeConfig {
            state_sync: StateSyncConfig {
                data_streaming_service: DataStreamingServiceConfig {
                    max_concurrent_state_requests: 100,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        // Create a local config YAML where the state prefetcher is set to 100
        let local_config_yaml = serde_yaml::from_str(
            r#"
            state_sync:
                data_streaming_service:
                    max_concurrent_state_requests: 100
            "#,
        )
        .unwrap();

        // Verify that the state prefetcher aggression has not changed
        // but the regular prefetcher aggression has doubled (for a VFN).
        StateSyncConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::ValidatorFullnode,
            ChainId::testnet(),
        )
        .unwrap();
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_requests,
            MAX_CONCURRENT_REQUESTS * 2,
        );
        assert_eq!(
            node_config
                .state_sync
                .data_streaming_service
                .max_concurrent_state_requests,
            100
        );
    }

    /// Creates and returns a node config with the syncing modes to execution
    fn create_execution_mode_config() -> NodeConfig {
        NodeConfig {
            state_sync: StateSyncConfig {
                state_sync_driver: StateSyncDriverConfig {
                    bootstrapping_mode: BootstrappingMode::ExecuteTransactionsFromGenesis,
                    continuous_syncing_mode: ContinuousSyncingMode::ExecuteTransactions,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Generates a local config YAML. If `set_bootstrapping_mode` is true,
    /// the bootstrapping mode is set to `ExecuteTransactionsFromGenesis`.
    /// Otherwise, an irrelevant field is set.
    fn generate_local_config_yaml(set_bootstrapping_mode: bool) -> Value {
        let yaml_string = if set_bootstrapping_mode {
            r#"
            state_sync:
                state_sync_driver:
                    bootstrapping_mode: ExecuteTransactionsFromGenesis
            "#
        } else {
            r#"
            state_sync:
                state_sync_driver:
                    irrelevant_field: true
            "#
        };
        serde_yaml::from_str(yaml_string).unwrap()
    }
}
