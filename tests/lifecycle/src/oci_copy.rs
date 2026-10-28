// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{pipeline::Pipeline, request::BuildRequest};
use std::{fs, path::Path, process::Command};

/// Preserve the exploratory pipeline's third-party OCI graph-copy assertion.
#[derive(Debug)]
pub(super) struct OciCopy;
impl OciCopy {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let context = Path::new("/work/oci-context");
        fs::create_dir(context)?;
        fs::write(
            context.join("plan.yaml"),
            "tasks:\n  rootfs:\n    kind: extend\n    from: qualification.local/fixture:ci\nimage: rootfs\n",
        )?;
        fs::write(
            context.join("images.lock"),
            format!(
                "version: 1\nimages:\n  qualification.local/fixture:ci:\n    index: {}\n",
                fs::read_to_string("/cache-fixture/chain.pin")?.trim()
            ),
        )?;
        for arguments in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=OCI test",
                "-c",
                "user.email=test@invalid",
                "commit",
                "-qm",
                "Pin OCI fixture",
            ],
        ] {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(context)
                    .args(arguments)
                    .status()?
                    .success()
            );
        }
        let commit = Command::new("git")
            .arg("-C")
            .arg(context)
            .args(["rev-parse", "HEAD"])
            .output()?;
        assert!(commit.status.success());
        let commit = String::from_utf8(commit.stdout)?;
        Pipeline::new(BuildRequest::parse_from([
            "conglobate",
            "build",
            "--repository",
            "/work/oci-context",
            "--commit",
            commit.trim(),
            "--plan",
            "plan.yaml",
            "--lock",
            "images.lock",
            "--cache",
            "/cache-fixture",
            "--output",
            "/work/oci-output",
        ])?)
        .run()?;
        let artifact = Path::new("/work/oci-output");
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(artifact.join("index.json"))?)?;
        let digest = index["manifests"][0]["digest"]
            .as_str()
            .ok_or("missing manifest digest")?;
        // Capture output so ORAS cannot leave terminal controls on the marker line.
        let copy = Command::new("/usr/bin/oras")
            .args([
                "cp",
                "--from-oci-layout",
                "--to-oci-layout",
                &format!("/work/oci-output@{digest}"),
                "/work/oras-copy:verified",
            ])
            .output()?;
        println!("{}", String::from_utf8_lossy(&copy.stdout));
        eprintln!("{}", String::from_utf8_lossy(&copy.stderr));
        assert!(copy.status.success());
        let copied = Path::new("/work/oras-copy");
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(copied.join("index.json"))?)?;
        assert_eq!(index["manifests"][0]["digest"].as_str(), Some(digest));
        let names =
            |root: &Path| -> std::io::Result<std::collections::BTreeSet<std::ffi::OsString>> {
                fs::read_dir(root.join("blobs/sha256"))?
                    .map(|entry| entry.map(|entry| entry.file_name()))
                    .collect()
            };
        let original = names(artifact)?;
        assert_eq!(original, names(copied)?);
        for name in original {
            assert_eq!(
                fs::read(artifact.join("blobs/sha256").join(&name))?,
                fs::read(copied.join("blobs/sha256").join(&name))?
            );
        }
        assert!(!artifact.join("inventory.cbor").exists());
        let manifest: serde_json::Value = serde_json::from_slice(&fs::read(
            copied
                .join("blobs/sha256")
                .join(digest.strip_prefix("sha256:").unwrap()),
        )?)?;
        let annotations = &manifest["annotations"];
        assert_eq!(
            annotations["dev.pichi.build.commit"],
            format!("sha1:{}", commit.trim())
        );
        assert_eq!(annotations["dev.pichi.build.context"], ".");
        assert_eq!(annotations["dev.pichi.build.plan"], "plan.yaml");
        assert_eq!(annotations["dev.pichi.build.lock"], "images.lock");
        assert!(annotations.get("dev.pichi.build.location").is_none());
        assert!(annotations.get("dev.pichi.carapace.verity.hash").is_none());
        println!("OCI_ORAS_SELF_CONTAINED_COPY_PASS");
        Ok(())
    }
}
