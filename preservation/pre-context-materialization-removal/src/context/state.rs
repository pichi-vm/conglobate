// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ReadOnlyResources, StagedResources};
mod private {
    pub trait Sealed {}
}

/// State implementations are sealed; callers cannot assert a mount transition.
pub trait CheckoutState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Staged;
#[derive(Debug)]
pub struct ReadOnly;
impl private::Sealed for Staged {}
impl private::Sealed for ReadOnly {}
impl CheckoutState for Staged {
    type Resources = StagedResources;
}
impl CheckoutState for ReadOnly {
    type Resources = ReadOnlyResources;
}

/// State-specific resource payloads own cleanup; the outer wrapper can move
/// them through consuming transitions without unsafe extraction or state flags.
///
/// ```compile_fail
/// use conglobate::context::{Checkout, ReadOnly};
/// fn cannot_freeze_twice(checkout: Checkout<ReadOnly>) {
///     let again = checkout.read_only();
/// }
/// ```
///
/// ```compile_fail
/// use conglobate::context::{Checkout, ReadOnly};
/// fn cannot_transfer_mount_ownership(checkout: Checkout<ReadOnly>) {
///     std::thread::spawn(move || drop(checkout));
/// }
/// ```
#[derive(Debug)]
pub struct Checkout<S: CheckoutState> {
    pub(super) resources: S::Resources,
}
pub type StagedContext = Checkout<Staged>;
pub type ReadOnlyContext = Checkout<ReadOnly>;
