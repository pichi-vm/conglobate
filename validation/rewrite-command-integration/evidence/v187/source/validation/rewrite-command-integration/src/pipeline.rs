// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
#[derive(Debug)]
pub(super) struct PipelineProbe;
impl PipelineProbe {
    fn build(output: &str, recipe: &str) -> std::io::Result<Output> {
        Command::new("/usr/bin/conglobate")
            .args([
                "build",
                "--repository",
                "/pipeline-context",
                "--commit",
                fs::read_to_string("/pipeline-commit")?.trim(),
                "--context",
                ".",
                "--recipe",
                recipe,
                "--lock",
                "images.lock",
                "--cache",
                "/cache-fixture",
                "--output",
                output,
            ])
            .output()
    }
    fn verify(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let index: serde_json::Value = serde_json::from_slice(&fs::read(path.join("index.json"))?)?;
        assert_eq!(index["manifests"].as_array().unwrap().len(), 1);
        let descriptor = &index["manifests"][0];
        let digest = descriptor["digest"]
            .as_str()
            .unwrap()
            .strip_prefix("sha256:")
            .unwrap();
        let bytes = fs::read(path.join("blobs/sha256").join(digest))?;
        assert_eq!(hex::encode(Sha256::digest(&bytes)), digest);
        assert_eq!(bytes.len() as u64, descriptor["size"].as_u64().unwrap());
        let manifest: serde_json::Value = serde_json::from_slice(&bytes)?;
        let mut references = std::collections::BTreeSet::from([digest.to_owned()]);
        for layer in manifest["layers"]
            .as_array()
            .unwrap()
            .iter()
            .chain(std::iter::once(&manifest["config"]))
        {
            let digest = layer["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap();
            references.insert(digest.to_owned());
            let bytes = fs::read(path.join("blobs/sha256").join(digest))?;
            assert_eq!(hex::encode(Sha256::digest(&bytes)), digest);
            assert_eq!(bytes.len() as u64, layer["size"].as_u64().unwrap());
        }
        assert_eq!(
            fs::read_dir(path.join("blobs/sha256"))?.count(),
            references.len()
        );
        let bytes = fs::read(path.join("inventory.cbor"))?;
        assert_eq!(
            conglobate::inventory::Inventory::decode(&bytes)?.version(),
            2
        );
        let inventory = Field(&bytes);
        assert_eq!(
            inventory.get(&["outputs", "manifest", "digest"])?.bytes()?,
            hex::decode(digest)?
        );
        assert_eq!(
            inventory
                .get(&["builder", "executable", "digest"])?
                .bytes()?,
            Sha256::digest(fs::read("/usr/bin/conglobate")?).as_slice()
        );
        assert_eq!(
            inventory.get(&["context", "commit"])?.bytes()?,
            hex::decode(fs::read_to_string("/pipeline-commit")?.trim())?
        );
        assert_eq!(
            inventory
                .get(&["execution", "recipe", "content", "digest"])?
                .bytes()?,
            Sha256::digest(fs::read("/pipeline-recipe-committed")?).as_slice()
        );
        assert_eq!(
            inventory.get(&["execution", "recipe", "path"])?.text()?,
            "recipes/image.yml"
        );
        assert_eq!(
            inventory.get(&["execution", "lock", "path"])?.text()?,
            "images.lock"
        );
        assert_eq!(
            inventory
                .get(&["execution", "transform", "source"])?
                .text()?,
            "builtin"
        );
        assert!(
            minicbor::Decoder::new(
                inventory
                    .get(&["execution", "transform", "default_skipped"])?
                    .0
            )
            .bool()?
        );
        assert!(
            !minicbor::Decoder::new(
                inventory
                    .get(&["execution", "transform", "target_present"])?
                    .0
            )
            .bool()?
        );
        assert_eq!(
            inventory.get(&["outputs", "carapace_root"])?.bytes()?,
            hex::decode(
                manifest["annotations"]["dev.pichi.carapace.verity.hash"]
                    .as_str()
                    .unwrap()
            )?
        );
        Ok(())
    }
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let output = Self::build("/work/pipeline-output", "recipes/image.yml")?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        Self::verify(Path::new("/work/pipeline-output"))?;
        let before = fs::read("/work/pipeline-output/index.json")?;
        assert!(
            !Self::build("/work/pipeline-output", "recipes/image.yml")?
                .status
                .success()
        );
        assert_eq!(fs::read("/work/pipeline-output/index.json")?, before);
        let unsupported = Self::build("/work/unsupported-output", "recipes/unsupported.yml")?;
        assert!(!unsupported.status.success());
        assert!(String::from_utf8_lossy(&unsupported.stderr).contains("not implemented"));
        assert!(!Path::new("/work/unsupported-output").exists());
        fs::remove_dir_all("/cache-fixture")?;
        Self::verify(Path::new("/work/pipeline-output"))?;
        assert!(
            !Self::build("/work/missing-output", "recipes/image.yml")?
                .status
                .success()
        );
        assert!(!Path::new("/work/missing-output").exists());
        assert!(!fs::read_dir("/work")?.any(|p| {
            p.unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".conglobate-output-")
        }));
        println!("PIPELINE_CLI_PASSTHROUGH_INVENTORY_PASS");
        println!("PIPELINE_CLI_FAILURE_NO_PUBLICATION_PASS");
        Ok(())
    }
}
#[derive(Debug)]
struct Field<'a>(&'a [u8]);
impl<'a> Field<'a> {
    fn get(&self, path: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut bytes = self.0;
        for key in path {
            let mut decoder = minicbor::Decoder::new(bytes);
            let count = decoder.map()?.ok_or("indefinite map")?;
            let mut selected = None;
            for _ in 0..count {
                let name = decoder.str()?;
                let start = decoder.position();
                decoder.skip()?;
                if name == *key {
                    selected = Some(&bytes[start..decoder.position()]);
                }
            }
            bytes = selected.ok_or("missing inventory field")?;
        }
        Ok(Self(bytes))
    }
    fn bytes(&self) -> Result<&'a [u8], minicbor::decode::Error> {
        minicbor::Decoder::new(self.0).bytes()
    }
    fn text(&self) -> Result<&'a str, minicbor::decode::Error> {
        minicbor::Decoder::new(self.0).str()
    }
}
