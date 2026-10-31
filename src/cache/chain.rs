// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, ResolvedImage};
use crate::artifact::{Digest as ContentDigest, Layer};
use carapace::{Chain, Scute};
use devmap_verity::{Algorithm, HashType, Header};
use rustix::fs::{Dir, OFlags};
use sha2::{Digest as _, Sha256};
use std::{
    os::unix::{ffi::OsStrExt as _, fs::FileExt as _},
    path::PathBuf,
};

/// Discover an ordered chain from authenticated index metadata and prepared files.
/// Discovery pins read-only files without activating devices. Assembly validates
/// their headers again; dm-verity checks payload blocks when they are read.
pub(crate) trait FromIndex: Sized {
    fn from_index(cache: &Cache, image: &ResolvedImage) -> Result<Self, CacheError>;
}

impl FromIndex for Chain {
    fn from_index(cache: &Cache, image: &ResolvedImage) -> Result<Self, CacheError> {
        let root = image
            .root()
            .ok_or(CacheError::Invalid("indexed image has no carapace"))?;
        let mut parent = root;
        let mut scutes = Vec::new();
        for layer in image.manifest().layers.iter().rev() {
            if !matches!(layer, Layer::Scute(_) | Layer::ScuteZstd(_)) {
                continue;
            }
            if parent == [0; 32] {
                return Err(CacheError::Invalid("scutes are not in base-to-tip order"));
            }
            let (scute, previous) = cache.prepared_scute(layer, parent)?;
            scutes.push(scute);
            parent = previous;
        }
        if scutes.is_empty() || parent != [0; 32] {
            return Err(CacheError::Invalid(
                "manifest does not contain a complete chain",
            ));
        }
        scutes.reverse();
        Ok(Self {
            root: root.into(),
            scutes,
        })
    }
}

impl Cache {
    fn prepared_scute(
        &self,
        layer: &Layer,
        root: [u8; 32],
    ) -> Result<(Scute, [u8; 32]), CacheError> {
        let digest: ContentDigest = layer
            .digest_str()
            .parse()
            .map_err(|_| CacheError::Invalid("invalid scute digest"))?;
        let source = PathBuf::from(format!("blobs/{}/{}", digest.algo(), digest.hex()));
        let directory = source.with_extension("prepared");
        let handle = self.open_at(&directory, OFlags::DIRECTORY)?;
        let mut selected = None;
        for entry in Dir::read_from(&handle).map_err(std::io::Error::from)? {
            let entry = entry.map_err(std::io::Error::from)?;
            let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
            let path = directory.join(name);
            if path
                .extension()
                .is_none_or(|extension| extension != "verity")
            {
                continue;
            }
            let hash = self.regular_file(&path)?;
            let mut record = [0; 512];
            hash.read_exact_at(&mut record, 0)?;
            let header = Header::decode(record)?;
            let parameters = &header.parameters;
            // The manifest tip is SHA-256. Inspect only the root tree block to
            // select a cached variant; do not reconstruct or scan the tree.
            if parameters.algorithm != Algorithm::Sha256 || parameters.hash_type != HashType::Normal
            {
                // Another image may have prepared this source with a different
                // hash convention. It cannot satisfy this SHA-256 chain tip.
                continue;
            }
            let parent: [u8; 32] = parameters
                .salt
                .as_ref()
                .get(..32)
                .ok_or(CacheError::Invalid(
                    "prepared scute salt has no parent root",
                ))?
                .try_into()
                .expect("checked parent length");
            let size = parameters.data_size()?;
            if hash.metadata()?.len() != header.total_size()? {
                return Err(CacheError::Invalid("prepared hash extent mismatch"));
            }
            let data_path = if matches!(layer, Layer::ScuteZstd(_)) || layer.size() != size {
                directory.join(format!("{size}.deflated"))
            } else {
                source.clone()
            };
            let data = self.regular_file(&data_path)?;
            if data.metadata()?.len() != size {
                return Err(CacheError::Invalid("prepared data extent mismatch"));
            }
            let single_block = header.tree_size()? == 0;
            let block_size = if single_block {
                parameters.data.size
            } else {
                parameters.hash
            };
            let mut block = vec![0; block_size.bytes().get() as usize];
            if single_block {
                data.read_exact_at(&mut block, 0)?;
            } else {
                hash.read_exact_at(&mut block, header.prefix_size())?;
            }
            let mut hasher = Sha256::new();
            hasher.update(parameters.salt.as_ref());
            hasher.update(&block);
            if hasher.finalize().as_slice() != root {
                continue;
            }
            if selected.replace((Scute { data, hash }, parent)).is_some() {
                return Err(CacheError::Invalid("ambiguous prepared scute variants"));
            }
        }
        selected.ok_or(CacheError::Invalid(
            "prepared scute does not match the chain root",
        ))
    }
}
