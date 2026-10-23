// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::CapturedFormatter;
use crate::{document::YamlDocument, path::ContextPath};
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Trusted environment input, separate from the committed image recipe.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FormatterManifest {
    root: PathBuf,
    executable: PathBuf,
    files: Vec<PathBuf>,
}
impl FormatterManifest {
    pub(crate) fn capture(path: &Path) -> io::Result<CapturedFormatter> {
        let text = fs::read_to_string(path)?;
        let manifest: Self = YamlDocument::from(text.as_str())
            .decode()
            .map_err(io::Error::other)?;
        let root = path.parent().unwrap_or(Path::new(".")).join(&manifest.root);
        let executable =
            ContextPath::try_from(manifest.executable.as_path()).map_err(io::Error::other)?;
        let paths = manifest
            .files
            .iter()
            .map(|path| ContextPath::try_from(path.as_path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        CapturedFormatter::capture(&root, executable, paths)?.with_ext4_configuration()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn manifest_captures_relative_runtime_and_rejects_ambiguous_inputs() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("runtime")).unwrap();
        fs::write(temp.path().join("runtime/tool"), b"tool bytes").unwrap();
        fs::set_permissions(
            temp.path().join("runtime/tool"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let path = temp.path().join("formatter.yml");
        fs::write(&path, "root: runtime\nexecutable: tool\nfiles: [tool]\n").unwrap();
        let captured = FormatterManifest::capture(&path).unwrap();
        assert!(captured.files().any(
            |(path, file)| path.as_ref() == Path::new("tool") && file.bytes() == b"tool bytes"
        ));
        for text in [
            "root: runtime\nroot: /\nexecutable: tool\nfiles: [tool]\n",
            "root: runtime\nexecutable: tool\nfiles: [tool, tool]\n",
            "root: runtime\nexecutable: tool\nfiles: [../tool]\n",
            "root: runtime\nexecutable: tool\nfiles: [tool, missing]\n",
            "root: runtime\nexecutable: tool\nfiles: [tool]\noptions: []\n",
        ] {
            fs::write(&path, text).unwrap();
            assert!(FormatterManifest::capture(&path).is_err());
        }
    }
}
