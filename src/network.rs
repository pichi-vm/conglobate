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
