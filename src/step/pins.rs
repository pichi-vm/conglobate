// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    anchor::{
        FilesystemPin, FilesystemPinReleaseFailure, FilesystemPinTarget, NamespacePin, PinTarget,
        ReleaseFailure,
    },
    isolation::{PinnedNamespace, UserNamespace, UserNamespaceReleaseFailure},
    policy::PolicyIdentities,
};
use std::{io, time::Instant};

#[derive(Debug)]
enum Filesystem {
    Mounted(FilesystemPin),
    Target(FilesystemPinTarget),
}
impl Filesystem {
    fn release(self) -> Result<(), (io::Error, Self)> {
        let result = match self {
            Self::Mounted(pin) => pin.release(),
            Self::Target(target) => target.cleanup(),
        };
        result.map_err(|failure| match failure {
            FilesystemPinReleaseFailure::Mounted { source, pin } => (source, Self::Mounted(pin)),
            FilesystemPinReleaseFailure::Target { source, target } => {
                (source, Self::Target(target))
            }
        })
    }
}
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
    filesystems: Vec<Filesystem>,
    namespace: Namespace,
}
impl From<PolicyIdentities> for PinCleanup {
    fn from(identities: PolicyIdentities) -> Self {
        let (namespace, filesystems) = identities.into_parts();
        Self {
            filesystems: filesystems.into_iter().map(Filesystem::Mounted).collect(),
            namespace: Namespace::Mapped(namespace),
        }
    }
}
#[derive(Debug)]
pub(super) struct PinFailure {
    pub source: io::Error,
    pub cleanup: PinCleanup,
}
impl PinCleanup {
    pub(super) fn target(target: PinTarget) -> Self {
        Self {
            filesystems: Vec::new(),
            namespace: Namespace::Target(target),
        }
    }
    pub(super) fn namespace(namespace: UserNamespace<PinnedNamespace>) -> Self {
        Self {
            filesystems: Vec::new(),
            namespace: Namespace::Mapped(namespace),
        }
    }
    pub(super) fn with_filesystem(mut self, pin: FilesystemPin) -> Self {
        self.filesystems.push(Filesystem::Mounted(pin));
        self
    }
    pub(super) fn with_target(mut self, target: FilesystemPinTarget) -> Self {
        self.filesystems.push(Filesystem::Target(target));
        self
    }

    pub(super) fn run(mut self, deadline: Instant) -> Result<(), Box<PinFailure>> {
        while !self.filesystems.is_empty() {
            if let Err(source) = super::teardown::Deadline::check(deadline) {
                return Err(Box::new(PinFailure {
                    source,
                    cleanup: self,
                }));
            }
            let pin = self.filesystems.pop().expect("nonempty pins");
            if let Err((source, pin)) = pin.release() {
                self.filesystems.push(pin);
                return Err(Box::new(PinFailure {
                    source,
                    cleanup: self,
                }));
            }
        }
        if let Err(source) = super::teardown::Deadline::check(deadline) {
            return Err(Box::new(PinFailure {
                source,
                cleanup: self,
            }));
        }
        self.namespace.release().map_err(|(source, namespace)| {
            Box::new(PinFailure {
                source,
                cleanup: Self {
                    filesystems: self.filesystems,
                    namespace,
                },
            })
        })
    }
}
