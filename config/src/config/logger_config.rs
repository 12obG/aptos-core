// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    config::{
        config_optimizer::ConfigOptimizer, config_sanitizer::ConfigSanitizer,
        node_config_loader::NodeType, utils::is_tokio_console_enabled, Error, NodeConfig,
    },
    utils,
};
use aptos_logger::{Level, CHANNEL_SIZE};
use aptos_types::chain_id::ChainId;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

// Useful constants for the logger config
const DEFAULT_TOKIO_CONSOLE_PORT: u16 = 6669;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggerConfig {
    // channel size for the asynchronous channel for node logging.
    pub chan_size: usize,
    // Enables backtraces on error logs
    pub enable_backtrace: bool,
    // Use async logging
    pub is_async: bool,
    // The default logging level for slog.
    pub level: Level,
    pub enable_telemetry_remote_log: bool,
    pub enable_telemetry_flush: bool,
    pub telemetry_level: Level,
    pub tokio_console_port: Option<u16>,
}

impl Default for LoggerConfig {
    fn default() -> LoggerConfig {
        LoggerConfig {
            chan_size: CHANNEL_SIZE,
            enable_backtrace: false,
            is_async: true,
            level: Level::Info,
            enable_telemetry_remote_log: true,
            enable_telemetry_flush: true,
            telemetry_level: Level::Error,

            // This is the default port used by tokio-console.
            // Setting this to None will disable tokio-console
            // even if the "tokio-console" feature is enabled.
            tokio_console_port: None,
        }
    }
}

impl LoggerConfig {
    pub fn disable_tokio_console(&mut self) {
        self.tokio_console_port = None;
    }

    pub fn randomize_ports(&mut self) {
        self.tokio_console_port = Some(utils::get_available_port());
    }
}

impl ConfigSanitizer for LoggerConfig {
    /// Validate and process the logger config according to the given node type and chain ID
    fn sanitize(
        node_config: &mut NodeConfig,
        _node_type: NodeType,
        _chain_id: ChainId,
    ) -> Result<(), Error> {
        let sanitizer_name = Self::get_sanitizer_name();
        let logger_config = &node_config.logger;

        // Verify that tokio console tracing is correctly configured
        if is_tokio_console_enabled() && logger_config.tokio_console_port.is_none() {
            return Err(Error::ConfigSanitizerFailed(
                sanitizer_name,
                "The tokio-console feature is enabled but the tokio console port is not set!"
                    .into(),
            ));
        } else if !is_tokio_console_enabled() && logger_config.tokio_console_port.is_some() {
            return Err(Error::ConfigSanitizerFailed(
                sanitizer_name,
                "The tokio-console feature is not enabled but the tokio console port is set!"
                    .into(),
            ));
        }

        Ok(())
    }
}

impl ConfigOptimizer for LoggerConfig {
    /// Optimize the logger config according to the given node type and chain ID
    fn optimize(
        node_config: &mut NodeConfig,
        local_config_yaml: &Value,
        _node_type: NodeType,
        _chain_id: ChainId,
    ) -> Result<(), Error> {
        let logger_config = &mut node_config.logger;
        let local_logger_config_yaml = &local_config_yaml["logger"];

        // Set the tokio console port
        if local_logger_config_yaml["tokio_console_port"].is_null() {
            // If the tokio-console feature is enabled, set the default port.
            // Otherwise, disable the tokio console port.
            if is_tokio_console_enabled() {
                logger_config.tokio_console_port = Some(DEFAULT_TOKIO_CONSOLE_PORT);
            } else {
                logger_config.tokio_console_port = None;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_tokio_console_port() {
        // Create a logger config with the tokio console port set
        let mut node_config = NodeConfig {
            logger: LoggerConfig {
                tokio_console_port: Some(100),
                ..Default::default()
            },
            ..Default::default()
        };

        // Create a local config YAML without any relevant fields
        let local_config_yaml = serde_yaml::from_str(
            r#"
            logger:
                irrelevant_field: true
            "#,
        )
        .unwrap();

        // Verify that the configuration is optimized successfully
        // and that the tokio console port is disabled (because
        // the feature flag is not enabled).
        LoggerConfig::optimize(
            &mut node_config,
            &local_config_yaml,
            NodeType::Validator,
            ChainId::testnet(),
        )
        .unwrap();
        assert!(node_config.logger.tokio_console_port.is_none());
    }

    #[test]
    fn test_sanitize_missing_feature() {
        // Create a logger config with the tokio console port set
        let mut node_config = NodeConfig {
            logger: LoggerConfig {
                tokio_console_port: Some(100),
                ..Default::default()
            },
            ..Default::default()
        };

        // Verify that the config fails sanitization (the tokio-console feature is missing!)
        let error =
            LoggerConfig::sanitize(&mut node_config, NodeType::Validator, ChainId::testnet())
                .unwrap_err();
        assert!(matches!(error, Error::ConfigSanitizerFailed(_, _)));
    }
}
