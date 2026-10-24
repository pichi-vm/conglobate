// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Joined, Supervision, joining::Packet};
use crate::{
    filesystem::{Detached, Idmapped, Mount},
    isolation::{CommandDevices, CommandSetup, Mapped, PrivateMountNamespace, UserNamespace},
    network::{CommandNetwork, ResolverTarget, ResolverView},
    storage::VolumeView,
};
use std::{fs::File, io, time::Instant};

/// Trusted helper input. The sender supplies a detached mount and its selected
/// user namespace; neither descriptor may reach image commands. This is not
/// admission or proof that the sender's volume is safe to release.
#[derive(Debug)]
pub(crate) struct VolumeInput {
    root: File,
    namespace: File,
    output: Option<File>,
    network: Option<(File, ResolverView, File)>,
}
impl Supervision<Joined> {
    /// Transfer a view through the already paired and joined control channel.
    /// The enclosing owner must retain the volume and mapped namespace until
    /// descendant quiescence. Sending is not permission to unmount the volume.
    pub(crate) fn send_volume_until(
        &self,
        view: &VolumeView<'_>,
        namespace: &UserNamespace<Mapped>,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Volume(view.descriptor()?.into(), namespace.descriptor()?.into())
            .send(&self.socket.0, deadline)
    }

    pub(crate) fn send_bootstrap_until(
        &self,
        root: &VolumeView<'_>,
        output: &VolumeView<'_>,
        namespace: &UserNamespace<Mapped>,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Bootstrap(
            root.descriptor()?.into(),
            output.descriptor()?.into(),
            namespace.descriptor()?.into(),
        )
        .send(&self.socket.0, deadline)
    }

    pub(crate) fn send_network_volume_until(
        &self,
        root: &VolumeView<'_>,
        output: Option<&VolumeView<'_>>,
        raw: &VolumeView<'_>,
        namespace: &UserNamespace<Mapped>,
        network: &CommandNetwork,
        deadline: Instant,
    ) -> io::Result<()> {
        let raw = raw.descriptor()?.into();
        let resolver = network.resolver_view()?.into_handle().into();
        let net = network.namespace().descriptor().try_clone()?.into();
        let user = namespace.descriptor()?.into();
        let packet = match output {
            Some(output) => Packet::BootstrapNetwork(
                root.descriptor()?.into(),
                output.descriptor()?.into(),
                user,
                net,
                resolver,
                raw,
            ),
            None => Packet::VolumeNetwork(root.descriptor()?.into(), user, net, resolver, raw),
        };
        packet.send(&self.socket.0, deadline)
    }

    /// Dedicated trusted helper, before awaiting admission. The volume packet
    /// cannot be mistaken for admission by the ordinary admission decoder.
    pub(crate) fn receive_volume_until(&self, deadline: Instant) -> io::Result<VolumeInput> {
        let (root, output, namespace, network) = match Packet::receive(&self.socket.0, deadline)? {
            Packet::Volume(root, namespace) => (root, None, namespace, None),
            Packet::Bootstrap(root, output, namespace) => {
                (root, Some(File::from(output)), namespace, None)
            }
            Packet::VolumeNetwork(root, user, network, resolver, raw) => (
                root,
                None,
                user,
                Some((
                    File::from(network),
                    ResolverView::from_handle(resolver.into())?,
                    File::from(raw),
                )),
            ),
            Packet::BootstrapNetwork(root, output, user, network, resolver, raw) => (
                root,
                Some(File::from(output)),
                user,
                Some((
                    File::from(network),
                    ResolverView::from_handle(resolver.into())?,
                    File::from(raw),
                )),
            ),
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
            network,
        })
    }
}
impl VolumeInput {
    pub(crate) fn assigned_roots(&self) -> crate::isolation::AssignedRoots {
        if self.output.is_some() {
            crate::isolation::AssignedRoots::Bootstrap
        } else {
            crate::isolation::AssignedRoots::Root
        }
    }
    /// Construct the helper's idmapped root and next namespace-entry input.
    /// On failure all still-detached mount descriptors close; no attached mount
    /// or started command exists. The supervisor retains independent backing.
    pub(crate) fn prepare(
        self,
        scope: &PrivateMountNamespace,
        devices: CommandDevices,
    ) -> io::Result<(Mount<'_, Idmapped>, CommandSetup, Option<ResolverTarget>)> {
        let output_namespace = self.namespace.try_clone()?;
        let setup = CommandSetup::new(self.namespace.try_clone()?, devices)?;
        // Create placeholders on the raw view before applying its UID mapping.
        // This helper is trusted and no image writer has been admitted. On any
        // setup/cleanup failure the supervisor rejects the writable build layer.
        let (setup, resolver) = match self.network {
            Some((network, view, raw)) => {
                // The supervisor clones this independent raw view from its
                // mounted root. Our fixture rejected cloning the received
                // detached view (EINVAL), and retaining resolver scaffolding
                // handles on the IDMAP view caused EBUSY. Keep cleanup handles
                // on this separate raw view instead.
                use std::os::unix::fs::MetadataExt;
                let expected = self.root.metadata()?;
                let actual = raw.metadata()?;
                if !actual.is_dir()
                    || (expected.dev(), expected.ino()) != (actual.dev(), actual.ino())
                {
                    return Err(io::Error::other(
                        "resolver cleanup view does not match command root",
                    ));
                }
                let target = ResolverTarget::resolve(raw).map_err(io::Error::other)?;
                (
                    setup
                        .with_network_handle(network)?
                        .with_resolver_view(view)?,
                    Some(target),
                )
            }
            None => (setup, None),
        };
        let root = Mount::<Detached>::from_detached(self.root, scope)?
            .idmap(self.namespace)
            .map_err(|failure| {
                io::Error::other(format!(
                    "command root ID mapping: {}",
                    failure.into_parts().0
                ))
            })?;
        let root = match self.output {
            Some(output) => {
                let output = Mount::<Detached>::from_detached(output, scope)?
                    .idmap(output_namespace)
                    .map_err(|failure| failure.into_parts().0)?;
                root.with_output(output)?
            }
            None => root,
        };
        Ok((root, setup, resolver))
    }
}
