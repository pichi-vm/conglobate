// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Active, Created, Mapping, MappingFailure};
use crate::cache::PreparedScute;
use std::{fs::File, io, num::NonZeroU64};

/// Read-time verified data, with its loop devices and backing retained. No raw
/// control or writable backing handle escapes. Drop requests deferred mapping
/// removal and loop autoclear; only close confirms successful release.
#[derive(Debug)]
pub struct VerityMapping<'s, 'w> {
    mapping: Mapping<Active>,
    inputs: Inputs<'s, 'w>,
}

#[derive(Debug)]
struct Inputs<'s, 'w> {
    hash: ReadOnlyLoop,
    data: ReadOnlyLoop,
    prepared: PreparedScute<'s, 'w>,
}

/// Error phases own exactly the resources still requiring cleanup. They never
/// authorize reuse of an incompletely activated or incompletely removed mapping.
#[derive(Debug)]
enum Cleanup<'s, 'w> {
    Mapping(MappingFailure, Inputs<'s, 'w>),
    Inputs(Inputs<'s, 'w>),
    Data(ReadOnlyLoop, PreparedScute<'s, 'w>),
    Prepared(PreparedScute<'s, 'w>),
}

#[derive(Debug, thiserror::Error)]
#[error("verity mapping: {source}")]
pub struct VerityFailure<'s, 'w> {
    source: io::Error,
    cleanup: Cleanup<'s, 'w>,
}
impl VerityFailure<'_, '_> {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn cleanup(self) -> Result<(), Self> {
        self.cleanup.close()
    }
}

impl<'s, 'w> PreparedScute<'s, 'w> {
    pub fn activate(self) -> Result<VerityMapping<'s, 'w>, VerityFailure<'s, 'w>> {
        let data = match ReadOnlyLoop::attach(self.files().0) {
            Ok(data) => data,
            Err(source) => {
                return Err(VerityFailure {
                    source,
                    cleanup: Cleanup::Prepared(self),
                });
            }
        };
        let hash = match ReadOnlyLoop::attach(self.files().1) {
            Ok(hash) => hash,
            Err(source) => {
                return Err(VerityFailure {
                    source,
                    cleanup: Cleanup::Data(data, self),
                });
            }
        };
        let inputs = Inputs {
            hash,
            data,
            prepared: self,
        };
        let mapping = match Mapping::<Created>::create() {
            Ok(mapping) => mapping,
            Err(source) => {
                return Err(VerityFailure {
                    source,
                    cleanup: Cleanup::Inputs(inputs),
                });
            }
        };
        // Parameters originate in the authenticated plan, not a mutable header.
        let target = (|| -> io::Result<_> {
            Ok(inputs
                .prepared
                .header()
                .builder(
                    inputs.data.id()?,
                    inputs.hash.id()?,
                    inputs.prepared.root().to_vec().into_boxed_slice(),
                )?
                .build())
        })();
        let active = match target {
            Ok(target) => mapping.activate(
                NonZeroU64::new(target.data_sectors()).expect("validated nonempty extent"),
                &target,
                true,
            ),
            Err(source) => Err(mapping.failed(source)),
        };
        match active {
            Ok(mapping) => Ok(VerityMapping { mapping, inputs }),
            Err(failure) => Err(Cleanup::mapping(failure, inputs)),
        }
    }
}

impl<'s, 'w> VerityMapping<'s, 'w> {
    /// Diagnostic identity, not an independent lifetime lease.
    pub fn id(&self) -> devmap_linux::DevId {
        self.mapping.id()
    }
    pub fn close(self) -> Result<(), VerityFailure<'s, 'w>> {
        match self.mapping.close() {
            Ok(()) => Cleanup::Inputs(self.inputs).close(),
            Err(failure) => Err(Cleanup::mapping(failure, self.inputs)),
        }
    }
}

impl<'s, 'w> Cleanup<'s, 'w> {
    fn mapping(failure: MappingFailure, inputs: Inputs<'s, 'w>) -> VerityFailure<'s, 'w> {
        VerityFailure {
            source: io::Error::new(failure.source_error().kind(), failure.to_string()),
            cleanup: Self::Mapping(failure, inputs),
        }
    }
    fn close(self) -> Result<(), VerityFailure<'s, 'w>> {
        match self {
            Self::Mapping(failure, inputs) => match failure.cleanup() {
                Ok(()) => Self::Inputs(inputs).close(),
                Err(failure) => Err(Self::mapping(failure, inputs)),
            },
            Self::Inputs(Inputs {
                hash,
                data,
                prepared,
            }) => match hash.close() {
                Ok(()) => Self::Data(data, prepared).close(),
                Err((source, hash)) => Err(VerityFailure {
                    source,
                    cleanup: Self::Inputs(Inputs {
                        hash,
                        data,
                        prepared,
                    }),
                }),
            },
            Self::Data(data, prepared) => match data.close() {
                Ok(()) => Self::Prepared(prepared).close(),
                Err((source, data)) => Err(VerityFailure {
                    source,
                    cleanup: Self::Data(data, prepared),
                }),
            },
            Self::Prepared(prepared) => {
                drop(prepared);
                Ok(())
            }
        }
    }
}

/// Storage-private: never expose the underlying descriptor's control authority.
/// Single configuration attempt; autoclear keeps backing alive for kernel users.
#[derive(Debug)]
struct ReadOnlyLoop(lodown::Device);
impl ReadOnlyLoop {
    fn attach(file: &File) -> io::Result<Self> {
        let length = file.metadata()?.len();
        if length == 0 || !length.is_multiple_of(4096) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid read-only loop extent",
            ));
        }
        let device = lodown::Device::open(lodown::Control::open()?.get_free()?)?;
        device.configure(
            file,
            4096,
            lodown::Configurable {
                read_only: true,
                writable: lodown::Writable {
                    autoclear: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        Ok(Self(device))
    }
    fn id(&self) -> io::Result<devmap_linux::DevId> {
        self.0.as_ref().metadata()?.try_into()
    }
    fn close(self) -> Result<(), (io::Error, Self)> {
        let detached = (|| -> io::Result<()> {
            lodown::Device::from(self.0.as_ref().try_clone()?).clear()?;
            match self.0.status() {
                Err(error) if error.raw_os_error() == Some(6) => Ok(()),
                Err(error) => Err(error),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::ResourceBusy,
                    "read-only loop detach remains deferred",
                )),
            }
        })();
        detached.map_err(|source| (source, self))
    }
}
