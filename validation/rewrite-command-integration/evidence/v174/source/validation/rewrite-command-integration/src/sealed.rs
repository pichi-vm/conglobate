// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    cache::{Cache, HostArchitecture},
    image::IndexDigest,
    isolation::PrivateMountNamespace,
    storage::{Created, Mapping, SealedScute, Unmounted, Volume, Workspace},
};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    num::NonZeroU64,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Debug)]
pub(super) struct SealProbe;
impl SealProbe {
    pub(super) fn reopen(
        sealed: &SealedScute<'_>,
        workspace: &Workspace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let store = FixtureCache::create(Path::new("/work/sealed-cache"))?;
        let descriptor = sealed.descriptor();
        let digest = descriptor
            .digest
            .strip_prefix("sha256:")
            .ok_or("bad digest")?;
        let mut output = fs::File::create(store.0.join("blobs/sha256").join(digest))?;
        assert_eq!(
            io::copy(&mut sealed.data_reader(), &mut output)?,
            descriptor.size
        );
        drop(output);
        let pin = store.image(sealed)?;
        let cache = Cache::open(&store.0)?;
        let image = cache.resolve(pin)?;
        let chain = cache
            .verify_chain(&image)?
            .ok_or("missing produced chain")?;
        assert_eq!(chain.root(), sealed.root());
        let active = chain
            .assemble(&cache, workspace)
            .map_err(|e| e.to_string())?;
        let mut inspection = workspace.scratch()?;
        inspection.set_len(8 * 1024 * 1024)?;
        let sectors =
            NonZeroU64::new(conglobate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &active,
                inspection.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let scope = PrivateMountNamespace::enter()?;
        let mounted = Volume::<Unmounted>::new(snapshot, &scope)
            .mount_ext4(Path::new("/work"))
            .map_err(|e| e.to_string())?;
        let device = mounted.root_metadata()?.dev();
        let directory = fs::read_dir("/work")?
            .find_map(|entry| {
                let entry = entry.ok()?;
                (entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-volume-")
                    && entry.metadata().ok()?.dev() == device)
                    .then(|| entry.path())
            })
            .ok_or("missing inspection mount")?;
        assert_eq!(fs::read(directory.join("state"))?, b"output\nsecond\n");
        let metadata = fs::metadata(directory.join("state"))?;
        assert_eq!((metadata.uid(), metadata.gid()), (123, 456));
        assert!(!directory.join("tool-state").exists());
        let label = Command::new("getfattr")
            .args(["--only-values", "-n", "security.selinux"])
            .arg(directory.join("state"))
            .output()?;
        assert!(label.status.success());
        assert_eq!(label.stdout, b"foreign_u:object_r:foreign_t:s0");
        mounted
            .unmount()
            .map_err(|e| e.to_string())?
            .into_snapshot()
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        scope.restore().map_err(|e| e.to_string())?;
        active.close().map_err(|e| e.to_string())?;
        println!("BOOTSTRAP_SEALED_CHAIN_REOPEN_PASS");
        Ok(())
    }
}
/// Fixture-only OCI metadata; production publication belongs to M6.
#[derive(Debug)]
struct FixtureCache(PathBuf);
impl FixtureCache {
    fn create(path: &Path) -> io::Result<Self> {
        fs::create_dir(path)?;
        fs::create_dir_all(path.join("blobs/sha256"))?;
        Ok(Self(path.to_owned()))
    }
    fn blob(&self, bytes: &[u8]) -> io::Result<String> {
        let digest = hex::encode(Sha256::digest(bytes));
        fs::write(self.0.join("blobs/sha256").join(&digest), bytes)?;
        Ok(format!("sha256:{digest}"))
    }
    fn image(&self, sealed: &SealedScute<'_>) -> Result<IndexDigest, Box<dyn std::error::Error>> {
        let descriptor = sealed.descriptor();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2, "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "artifactType": "application/vnd.pichi.artifact.v1+json",
            "config": {"mediaType": "application/vnd.oci.empty.v1+json", "digest": self.blob(b"{}")?, "size": 2, "data": "e30="},
            "layers": [{"mediaType": "application/vnd.pichi.scute.v1", "digest": descriptor.digest, "size": descriptor.size,
                "annotations": {"dev.pichi.scute.verity.salt": descriptor.annotations.salt,
                    "dev.pichi.scute.verity.data-blocks": descriptor.annotations.data_blocks.unwrap().to_string()}}],
            "annotations": {"dev.pichi.carapace.verity.algo": "sha256", "dev.pichi.carapace.verity.data-block-size": "4096",
                "dev.pichi.carapace.verity.hash-block-size": "4096", "dev.pichi.carapace.verity.version": "1",
                "dev.pichi.carapace.verity.hash-type": "1",
                "dev.pichi.carapace.verity.hash": hex::encode(sealed.root())}
        }))?;
        let index = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [{"mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": "application/vnd.pichi.artifact.v1+json", "digest": self.blob(&manifest)?, "size": manifest.len(),
                "platform": {"os": "pichi", "architecture": HostArchitecture::current()?.oci_name()}}]
        }))?;
        Ok(self.blob(&index)?.parse()?)
    }
}
