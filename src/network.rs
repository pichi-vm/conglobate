// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Captured network inputs. Link/NAT and resolver-mount execution are separate
//! ownership gates; capturing configuration does not enable a command network.
mod resolver;
pub use resolver::ResolverInput;

mod route;
mod veth;
pub use veth::{ConnectedVeth, ConnectionFailure, InterfaceName, VethFailure, VethPair};

mod namespace;
pub use namespace::{NetworkNamespace, NetworkNamespaceFailure};

mod configuration;
pub(crate) use configuration::LinkConfiguration;
pub use configuration::NetworkAddress;

pub use veth::{ConfigurationCleanupFailure, ConfigurationFailure, ConfiguredVeth};

mod loopback;
pub(crate) use loopback::Loopback;

mod firewall;
mod netfilter;
pub use firewall::{FirewallCloseFailure, FirewallTable};

mod nat;
pub use nat::{NatCloseFailure, NatPolicy};

mod resolver_target;
pub use resolver_target::{ResolverTarget, ResolverTargetFailure};

mod resolver_mount;
pub use resolver_mount::{ResolverMount, ResolverSource, ResolverSourceFailure};
