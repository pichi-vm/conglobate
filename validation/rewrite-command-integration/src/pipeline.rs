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
    pub(super) fn derived(
        pin: conglobate::image::IndexDigest,
        workspace: &conglobate::storage::Workspace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            cache::Cache,
            isolation::PrivateMountNamespace,
            storage::{Created, Mapping, Unmounted, Volume},
        };
        use std::{num::NonZeroU64, os::unix::fs::MetadataExt};
        let repo = Path::new("/work/derive-context");
        fs::create_dir(repo)?;
        fs::write(repo.join("input"), b"committed copy\n")?;
        let recipe = "carapace:\n  from: example.test/tools:1\n  derive:\n    - run: 'echo first > /derived; test ! -e /work/derive-context'\n    - copy: {from: input, into: /copied}\n    - run: 'test $(cat /derived) = first; test \"$(cat /copied)\" = \"committed copy\"; echo final > /derived'\n";
        fs::write(repo.join("build.yml"), recipe)?;
        fs::write(
            repo.join("fail.yml"),
            "carapace:\n  from: example.test/tools:1\n  derive:\n    - run: 'echo rejected > /rejected; exit 37'\n",
        )?;
        fs::write(
            repo.join("images.lock"),
            format!(
                "version: 1\nimages:\n  example.test/tools:1:\n    index: sha256:{}\n",
                hex::encode(pin.bytes())
            ),
        )?;
        let dtb = Self::empty_dtb();
        fs::write(repo.join("firmware.pmi"), b"custom firmware output")?;
        fs::write(repo.join("firmware.dtb"), &dtb)?;
        let cache = Cache::open(Path::new("/work/tool-cache-success"))?;
        let image = cache.resolve(pin)?;
        let base_root = image
            .manifest()
            .carapace_verity_hash()
            .ok_or("missing tool root")?;
        for (name, base, missing) in [
            ("firmware-only", false, false),
            ("firmware-base", true, false),
            ("firmware-missing", false, true),
        ] {
            let check = if base {
                format!("test \"$CARAPACE\" = {}", hex::encode(base_root))
            } else {
                "test -z \"${CARAPACE+x}\"".into()
            };
            let mut document = serde_json::json!({"firmware": {
                "derive": [
                    {"copy":{"from":"firmware.pmi","into":"/out/image.pmi"}},
                    {"copy":{"from":"firmware.dtb","into":"/out/image.dtb"}},
                    {"run":format!("{check}; ln -s /out /alias; echo disposable > /firmware-tool-change")}
                ],
                "into":{"pmi":"/alias/image.pmi","dtb":if missing {"/missing.dtb"} else {"/alias/image.dtb"}}
            }});
            if base {
                document["carapace"] = serde_json::json!({"from":"example.test/tools:1"});
            } else {
                document["firmware"]["from"] = "example.test/tools:1".into();
            }
            fs::write(
                repo.join(format!("{name}.yml")),
                serde_json::to_vec(&document)?,
            )?;
        }
        for args in [
            vec!["init", "--quiet"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "CLI derive",
            ],
        ] {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(repo)
                    .args(args)
                    .status()?
                    .success()
            );
        }
        let commit = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()?;
        assert!(commit.status.success());
        let commit = String::from_utf8(commit.stdout)?;
        fs::write(repo.join("input"), b"uncommitted wrong bytes\n")?;
        for (recipe, output, success) in [
            ("build.yml", "/work/derived-output", true),
            ("fail.yml", "/work/rejected-output", false),
            ("firmware-only.yml", "/work/firmware-only-output", true),
            ("firmware-base.yml", "/work/firmware-base-output", true),
            (
                "firmware-missing.yml",
                "/work/firmware-missing-output",
                false,
            ),
        ] {
            let result = Command::new("/usr/bin/conglobate")
                .args([
                    "build",
                    "--repository",
                    repo.to_str().unwrap(),
                    "--commit",
                    commit.trim(),
                    "--recipe",
                    recipe,
                    "--lock",
                    "images.lock",
                    "--cache",
                    "/work/tool-cache-success",
                    "--output",
                    output,
                ])
                .output()?;
            assert_eq!(
                result.status.success(),
                success,
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(Path::new(output).exists(), success);
            assert_eq!(
                fs::read_dir("/run")?
                    .filter_map(Result::ok)
                    .filter(|e| e
                        .file_name()
                        .as_encoded_bytes()
                        .starts_with(b"conglobate-runtime-"))
                    .count(),
                0
            );
        }
        for (name, count) in [("firmware-only", 0), ("firmware-base", 1)] {
            let path = Path::new("/work").join(format!("{name}-output"));
            let index: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("index.json"))?)?;
            let digest = index["manifests"][0]["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap();
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("blobs/sha256").join(digest))?)?;
            let layers = manifest["layers"].as_array().unwrap();
            assert_eq!(layers.len(), count + 2);
            for (kind, expected) in [
                ("pmi", b"custom firmware output".as_slice()),
                ("dtb", dtb.as_slice()),
            ] {
                let layer = layers
                    .iter()
                    .find(|l| l["mediaType"] == format!("application/vnd.pichi.{kind}.v1"))
                    .unwrap();
                let digest = layer["digest"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("sha256:")
                    .unwrap();
                assert_eq!(fs::read(path.join("blobs/sha256").join(digest))?, expected);
                assert_eq!(digest, hex::encode(Sha256::digest(expected)));
            }
            let inventory = fs::read(path.join("inventory.cbor"))?;
            conglobate::inventory::Inventory::decode(&inventory)?;
            assert_eq!(
                minicbor::Decoder::new(Field(&inventory).get(&["outputs", "scutes"])?.0).array()?,
                Some(count as u64)
            );
            if count == 1 {
                assert_eq!(
                    manifest["annotations"]["dev.pichi.carapace.verity.hash"],
                    hex::encode(base_root)
                );
                assert_eq!(
                    Field(&inventory)
                        .get(&["outputs", "carapace_root"])?
                        .bytes()?,
                    base_root
                );
            } else {
                assert!(
                    Field(&inventory)
                        .get(&["outputs", "carapace_root"])
                        .is_err()
                );
                assert!(manifest["annotations"]["dev.pichi.carapace.verity.hash"].is_null());
            }
        }
        println!("PIPELINE_CLI_FIRMWARE_PAIR_PASS");
        let output = Path::new("/work/derived-output");
        let inventory = fs::read(output.join("inventory.cbor"))?;
        conglobate::inventory::Inventory::decode(&inventory)?;
        let entries = Field(&inventory).get(&["outputs", "scutes"])?;
        let mut decoder = minicbor::Decoder::new(entries.0);
        assert_eq!(decoder.array()?, Some(4));
        for expected in ["inherited", "produced", "produced", "produced"] {
            let start = decoder.position();
            decoder.skip()?;
            assert_eq!(
                Field(&entries.0[start..decoder.position()])
                    .get(&["origin"])?
                    .text()?,
                expected
            );
        }
        // Install only the index lookup blob for the cache reader, after checking
        // the published inventory. Production output contains no such extra blob.
        let index = fs::read(output.join("index.json"))?;
        let digest = hex::encode(Sha256::digest(&index));
        fs::write(output.join("blobs/sha256").join(&digest), &index)?;
        let cache = Cache::open(output)?;
        let image = cache.resolve(format!("sha256:{digest}").parse()?)?;
        cache.verify_references(&image)?;
        let chain = cache.verify_chain(&image)?.ok_or("missing derived chain")?;
        assert_eq!(chain.scutes().len(), 4);
        let active = chain
            .assemble(&cache, workspace)
            .map_err(|e| e.to_string())?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(8 * 1024 * 1024)?;
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &active,
                scratch.attach().map_err(|e| e.to_string())?,
                NonZeroU64::new(conglobate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512)
                    .unwrap(),
            )
            .map_err(|e| e.to_string())?;
        let scope = PrivateMountNamespace::enter()?;
        let volume = Volume::<Unmounted>::new(snapshot, &scope)
            .mount_ext4(Path::new("/work"))
            .map_err(|e| e.to_string())?;
        let device = volume.root_metadata()?.dev();
        let directory = fs::read_dir("/work")?
            .filter_map(Result::ok)
            .find(|e| {
                e.file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-volume-")
                    && e.metadata().is_ok_and(|m| m.dev() == device)
            })
            .ok_or("missing output mount")?
            .path();
        assert_eq!(fs::read(directory.join("derived"))?, b"final\n");
        assert_eq!(fs::read(directory.join("copied"))?, b"committed copy\n");
        assert!(!directory.join("rejected").exists());
        volume
            .unmount()
            .map_err(|e| e.to_string())?
            .into_snapshot()
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        scope.restore().map_err(|e| e.to_string())?;
        active.close().map_err(|e| e.to_string())?;
        println!("PIPELINE_CLI_DERIVE_PASS");
        Ok(())
    }
    fn empty_dtb() -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [0xd00dfeedu32, 72, 56, 72, 40, 17, 16, 0, 0, 16] {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        bytes.extend_from_slice(&[0; 16]);
        for word in [1u32, 0, 2, 9] {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        bytes
    }
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
    fn verify(
        path: &Path,
        recipe: &str,
        transformed: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
            Sha256::digest(if transformed {
                fs::read(Path::new("/pipeline-context").join(recipe))?
            } else {
                fs::read("/pipeline-recipe-committed")?
            })
            .as_slice()
        );
        assert_eq!(
            inventory.get(&["execution", "recipe", "path"])?.text()?,
            recipe
        );
        assert_eq!(
            inventory.get(&["execution", "lock", "path"])?.text()?,
            "images.lock"
        );
        assert_eq!(
            inventory
                .get(&["execution", "transform", "source"])?
                .text()?,
            if recipe == "recipes/authored.yml" {
                "authored"
            } else {
                "builtin"
            }
        );
        assert_eq!(
            minicbor::Decoder::new(
                inventory
                    .get(&["execution", "transform", "default_skipped"])?
                    .0
            )
            .bool()?,
            !transformed
        );
        assert_eq!(
            minicbor::Decoder::new(
                inventory
                    .get(&["execution", "transform", "target_present"])?
                    .0
            )
            .bool()?,
            transformed
        );
        assert_eq!(
            inventory.get(&["outputs", "carapace_root"])?.bytes()?,
            hex::decode(
                manifest["annotations"]["dev.pichi.carapace.verity.hash"]
                    .as_str()
                    .unwrap()
            )?
        );
        if transformed {
            assert_eq!(manifest["x-preserved"]["value"], 42);
            let descriptor = manifest["layers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["mediaType"] == "application/vnd.pichi.dtb.v1")
                .unwrap();
            let digest = descriptor["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap();
            let dtb = fs::read(path.join("blobs/sha256").join(digest))?;
            assert_eq!(
                inventory.get(&["outputs", "dtb", "digest"])?.bytes()?,
                hex::decode(digest)?
            );
            // Independent fixture encoding: find the expected complete NUL-terminated string.
            let root = manifest["annotations"]["dev.pichi.carapace.verity.hash"]
                .as_str()
                .unwrap();
            let expected = if recipe == "recipes/authored.yml" {
                format!("authored {root}\0")
            } else {
                format!("quiet corium={root}\0")
            };
            assert!(
                dtb.windows(expected.len())
                    .any(|w| w == expected.as_bytes())
            );
            assert_eq!(&dtb[28..32], &7u32.to_be_bytes());
        }
        Ok(())
    }
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let output = Self::build("/work/pipeline-output", "recipes/image.yml")?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        Self::verify(
            Path::new("/work/pipeline-output"),
            "recipes/image.yml",
            false,
        )?;
        let before = fs::read("/work/pipeline-output/index.json")?;
        println!("PIPELINE_INDEX_HEX {}", hex::encode(&before));
        println!(
            "PIPELINE_INVENTORY_HEX {}",
            hex::encode(fs::read("/work/pipeline-output/inventory.cbor")?)
        );
        let index: serde_json::Value = serde_json::from_slice(&before)?;
        let manifest = index["manifests"][0]["digest"]
            .as_str()
            .unwrap()
            .strip_prefix("sha256:")
            .unwrap();
        println!(
            "PIPELINE_MANIFEST_HEX {}",
            hex::encode(fs::read(
                Path::new("/work/pipeline-output/blobs/sha256").join(manifest)
            )?)
        );
        assert!(
            !Self::build("/work/pipeline-output", "recipes/image.yml")?
                .status
                .success()
        );
        assert_eq!(fs::read("/work/pipeline-output/index.json")?, before);
        let unsupported = Self::build("/work/unsupported-output", "recipes/unsupported.yml")?;
        assert!(!unsupported.status.success());
        assert!(!String::from_utf8_lossy(&unsupported.stdout).contains("MUST_NOT_RUN"));
        assert!(!Path::new("/work/unsupported-output").exists());
        for name in ["default", "authored"] {
            let destination = format!("/work/{name}-output");
            let recipe = format!("recipes/{name}.yml");
            let result = Self::build(&destination, &recipe)?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            Self::verify(Path::new(&destination), &recipe, true)?;
        }
        for (name, succeeds) in [("opaque-changed", false), ("opaque-unchanged", true)] {
            let destination = format!("/work/{name}-output");
            let result = Self::build(&destination, &format!("recipes/{name}.yml"))?;
            assert_eq!(
                result.status.success(),
                succeeds,
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(Path::new(&destination).exists(), succeeds);
            if !succeeds {
                assert!(
                    String::from_utf8_lossy(&result.stderr).contains("firmware/DTB compatibility")
                );
            }
        }
        println!("PIPELINE_FIRMWARE_DTB_CONTRACT_PASS");
        for name in ["missing-property", "missing-dtb"] {
            let destination = format!("/work/{name}-output");
            let result = Self::build(&destination, &format!("recipes/{name}.yml"))?;
            assert!(!result.status.success());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(if name == "missing-dtb" {
                    "requires an inherited or generated DTB"
                } else {
                    "missing DTB property"
                })
            );
            assert!(!Path::new(&destination).exists());
        }
        println!("PIPELINE_CLI_TRANSFORM_PASS");
        fs::remove_dir_all("/cache-fixture")?;
        Self::verify(
            Path::new("/work/pipeline-output"),
            "recipes/image.yml",
            false,
        )?;
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
