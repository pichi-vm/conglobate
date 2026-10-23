// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! External network handles and temporary resolver views. The environment owns
//! enabled networking; conglobate configures only isolated offline loopback.
mod resolver;
pub use resolver::ResolverInput;
mod loopback;
mod route;
pub(crate) use loopback::Loopback;
mod namespace;
pub use namespace::NetworkNamespace;
mod resolver_target;
pub use resolver_target::{ResolverTarget, ResolverTargetFailure};
mod resolver_mount;
pub(crate) use resolver_mount::ResolverView;
pub use resolver_mount::{ResolverMount, ResolverSource, ResolverSourceFailure};
mod command;
pub use command::{CommandNetwork, NetworkCleanupFailure};
mod setup;
pub use setup::{CommandNetworking, NetworkInput};
