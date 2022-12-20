// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

// Increase recursion limit for `serde_json::json!` macro parsing
#![recursion_limit = "256"]

// Need to use this for because src/schema.rs uses the macros and is autogenerated
#[macro_use]
extern crate diesel;

pub mod models;
pub mod schema;
pub mod util;
pub mod worker;
