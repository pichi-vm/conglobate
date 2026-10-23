// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Registered, Supervision, registration::Packet};
use crate::{
    filesystem::{Detached, Idmapped, Mount},
    isolation::{
        CommandDevices, CommandSetup, PinnedNamespace, PrivateMountNamespace, UserNamespace,
    },
    storage::VolumeView,
};
use std::{fs::File, io, time::Instant};

/// Trusted helper input. The sender supplies a detached mount and its selected
/// user namespace; neither descriptor may reach image commands. This is not
/// admission or proof that the sender's volume is safe to release.
#[derive(Debug)]
pub struct VolumeInput {
    root: File,
    namespace: File,
    output: Option<File>,
}
impl Supervision<Registered> {
    /// Transfer a view through the already paired and registered control channel.
    /// The enclosing owner must retain the volume and policy identities until
    /// descendant quiescence. Sending is not permission to unmount the volume.
    pub(crate) fn send_volume_until(
        &self,
        view: &VolumeView<'_>,
        namespace: &UserNamespace<PinnedNamespace>,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Volume(view.descriptor()?.into(), namespace.descriptor()?.into())
            .send(&self.socket.0, deadline)
    }

    pub(crate) fn send_bootstrap_until(
        &self,
        root: &VolumeView<'_>,
        output: &VolumeView<'_>,
        namespace: &UserNamespace<PinnedNamespace>,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Bootstrap(
            root.descriptor()?.into(),
            output.descriptor()?.into(),
            namespace.descriptor()?.into(),
        )
        .send(&self.socket.0, deadline)
    }

    /// Dedicated trusted helper, before awaiting admission. The volume packet
    /// cannot be mistaken for admission by the ordinary admission decoder.
    pub fn receive_volume_until(&self, deadline: Instant) -> io::Result<VolumeInput> {
        let (root, output, namespace) = match Packet::receive(&self.socket.0, deadline)? {
            Packet::Volume(root, namespace) => (root, None, namespace),
            Packet::Bootstrap(root, output, namespace) => {
                (root, Some(File::from(output)), namespace)
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "expected volume input",
                ));
            }
        };
        if let Some(output) = &output
            && !output.metadata()?.is_dir()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid output mount",
            ));
        }
        let root = File::from(root);
        let namespace = File::from(namespace);
        if !root.metadata()?.is_dir()
            || rustix::fs::fstatfs(&namespace)?.f_type as u64 != 0x6e73_6673
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid volume input objects",
            ));
        }
        Ok(VolumeInput {
            root,
            namespace,
            output,
        })
    }
}
impl VolumeInput {
    /// Construct the helper's idmapped root and next namespace-entry input.
    /// On failure all still-detached mount descriptors close; no attached mount
    /// or started command exists. The supervisor retains independent backing.
    pub fn prepare(
        self,
        scope: &PrivateMountNamespace,
        devices: CommandDevices,
    ) -> io::Result<(Mount<'_, Idmapped>, CommandSetup)> {
        let output_namespace = self.namespace.try_clone()?;
        let setup = CommandSetup::new(self.namespace.try_clone()?, devices)?;
        let root = Mount::<Detached>::from_detached(self.root, scope)?
            .idmap(self.namespace)
            .map_err(|failure| failure.into_parts().0)?;
        let root = match self.output {
            Some(output) => {
                let output = Mount::<Detached>::from_detached(output, scope)?
                    .idmap(output_namespace)
                    .map_err(|failure| failure.into_parts().0)?;
                root.with_output(output)?
            }
            None => root,
        };
        Ok((root, setup))
    }
}
