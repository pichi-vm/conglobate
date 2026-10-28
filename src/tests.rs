// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

// Binary-local contract tests; no production library API.
#[path = "../tests/unit/anchors.rs"]
mod anchors;
#[path = "../tests/unit/cache.rs"]
mod cache;
#[path = "../tests/unit/chains.rs"]
mod chains;
#[path = "../tests/unit/contexts.rs"]
mod contexts;
#[path = "../tests/unit/copy_modes.rs"]
mod copy_modes;
#[path = "../tests/unit/copy_patterns.rs"]
mod copy_patterns;
#[path = "../tests/unit/graphs.rs"]
mod graphs;
#[path = "../tests/unit/identities.rs"]
mod identities;
#[path = "../tests/unit/locks.rs"]
mod locks;
#[path = "../tests/unit/plans.rs"]
mod plans;
#[path = "../tests/unit/processes.rs"]
mod processes;
#[path = "../tests/unit/requests.rs"]
mod requests;
#[path = "../tests/unit/supervision.rs"]
mod supervision;
#[path = "../tests/unit/templates.rs"]
mod templates;
#[path = "../tests/unit/values.rs"]
mod values;
