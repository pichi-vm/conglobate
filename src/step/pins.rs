// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    anchor::{NamespacePin, PinTarget, ReleaseFailure},
    isolation::{PinnedNamespace, UserNamespace, UserNamespaceReleaseFailure},
};
use std::{io, time::Instant};

#[derive(Debug)]
enum Namespace {
    Mapped(UserNamespace<PinnedNamespace>),
    Raw(NamespacePin),
    Target(PinTarget),
}
impl Namespace {
    fn release(self) -> Result<(), (io::Error, Self)> {
        match self {
            Self::Mapped(namespace) => namespace.release().map_err(|failure| match failure {
                UserNamespaceReleaseFailure::Mounted { source, namespace } => {
                    (source, Self::Mapped(namespace))
                }
                UserNamespaceReleaseFailure::Target { source, target } => {
                    (source, Self::Target(target))
                }
            }),
            Self::Raw(pin) => Self::anchor_result(pin.release()),
            Self::Target(target) => Self::anchor_result(target.cleanup()),
        }
    }
    fn anchor_result(result: Result<(), ReleaseFailure>) -> Result<(), (io::Error, Self)> {
        result.map_err(|failure| match failure {
            ReleaseFailure::Mounted { source, pin } => (source, Self::Raw(pin)),
            ReleaseFailure::Target { source, target } => (source, Self::Target(target)),
        })
    }
}
#[derive(Debug)]
pub(super) struct PinCleanup {
    namespace: Namespace,
}
#[derive(Debug)]
pub(super) struct PinFailure {
    pub source: io::Error,
    pub cleanup: PinCleanup,
}
impl PinCleanup {
    pub(super) fn target(target: PinTarget) -> Self {
        Self {
            namespace: Namespace::Target(target),
        }
    }
    pub(super) fn namespace(namespace: UserNamespace<PinnedNamespace>) -> Self {
        Self {
            namespace: Namespace::Mapped(namespace),
        }
    }
    pub(super) fn run(self, deadline: Instant) -> Result<(), Box<PinFailure>> {
        if let Err(source) = super::teardown::Deadline::check(deadline) {
            return Err(Box::new(PinFailure {
                source,
                cleanup: self,
            }));
        }
        self.namespace.release().map_err(|(source, namespace)| {
            Box::new(PinFailure {
                source,
                cleanup: Self { namespace },
            })
        })
    }
}
