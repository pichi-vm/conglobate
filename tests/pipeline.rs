// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod common;
#[path = "common/firmware.rs"]
mod firmware_fixture;
use conglobate::{
    inventory::{Digest, Inventory},
    mounts::Scratch,
    scute::Scute,
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn blob(cache: &Path, bytes: &[u8]) -> String {
    let digest = Digest::sha256(bytes).oci();
    fs::write(cache.join("blobs/sha256").join(&digest[7..]), bytes).unwrap();
    digest
}

fn seed(directory: &Path) -> (std::path::PathBuf, String) {
    let root = directory.join("seed-root");
    fs::create_dir(&root).unwrap();
    common::install(&root, "/bin/sh");
    fs::write(root.join("tool-only"), "tool input").unwrap();
    let raw = directory.join("raw");
    fs::File::create(&raw)
        .unwrap()
        .set_len(32 * 1024 * 1024)
        .unwrap();
    assert!(
        Command::new("mkfs.ext4")
            .args(["-q", "-F", "-b", "4096", "-d"])
            .arg(&root)
            .arg(&raw)
            .status()
            .unwrap()
            .success()
    );
    let scute = Scute::import(&raw, &directory.join("seed-scute")).unwrap();
    let mut annotations = BTreeMap::new();
    for (key, value) in [
        ("algo", "sha256"),
        ("data-block-size", "4096"),
        ("hash-block-size", "4096"),
        ("version", "1"),
        ("hash-type", "1"),
    ] {
        annotations.insert(format!("dev.pichi.carapace.verity.{key}"), value.into());
    }
    annotations.insert(
        "dev.pichi.carapace.verity.hash".into(),
        hex::encode(scute.root),
    );
    annotations.insert("example.inherited".into(), "yes".into());
    let manifest = Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        layers: vec![Layer::Scute(scute.descriptor.clone())],
        annotations,
    };
    let cache = directory.join("cache");
    fs::create_dir_all(cache.join("blobs/sha256")).unwrap();
    fs::copy(
        scute.data_path(),
        cache
            .join("blobs/sha256")
            .join(&scute.descriptor.digest[7..]),
    )
    .unwrap();
    let bytes = manifest.to_bytes().unwrap();
    let digest = blob(&cache, &bytes);
    let architecture = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    };
    let index = serde_json::json!({"schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [{"mediaType": "application/vnd.oci.image.manifest.v1+json", "artifactType": pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "digest": digest,
            "size": bytes.len(), "platform": {"os": "pichi", "architecture": architecture}}]});
    let index = blob(&cache, &serde_json::to_vec(&index).unwrap());
    (cache, index)
}

#[test]
#[ignore = "requires root, mkfs.ext4, device-mapper and Linux namespaces"]
fn build_cli_layers_replaces_firmware_and_publishes_bound_inventory() {
    let scratch = Scratch::new().unwrap();
    let (cache, index) = seed(scratch.path());
    let repository = scratch.path().join("repo");
    fs::create_dir_all(repository.join("pichi.build")).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(repository.join("message"), "copied input").unwrap();
    fs::write(
        repository.join("firmware.pmi"),
        firmware_fixture::image(".pmi.vm", &firmware_fixture::detached(), 65536),
    )
    .unwrap();
    let mut tree = devtree::OwnedTree::new(devtree::OwnedNode::new(""));
    let mut chosen = devtree::OwnedNode::new("chosen");
    chosen.set_property(devtree::OwnedProperty::new("bootargs").with_str("quiet corium=old"));
    tree.root_mut().set_child(chosen);
    fs::write(repository.join("base.dtb"), tree.encode().unwrap()).unwrap();
    fs::write(
        repository.join("pichi.build/refs.lock"),
        format!("version: 1\nimages:\n  tool:\n    index: {index}\n"),
    )
    .unwrap();
    fs::write(repository.join("pichi.build/carapace.yaml"), "from: tool\nderive:\n  - copy:\n      from: message\n      into: /message\n  - run: 'test -f /message; printf derived > /derived'\n").unwrap();
    fs::write(repository.join("pichi.build/firmware.yaml"), "from: tool\nderive:\n  - copy:\n      from: firmware.pmi\n      into: /boot.pmi\n  - copy:\n      from: base.dtb\n      into: /boot.dtb\n  - run: 'test ! -e /derived; printf %s \"$CARAPACE\" >> /boot.pmi'\ninto:\n  pmi: /boot.pmi\n  dtb: /boot.dtb\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "context"]);
    let commit = git(&repository, &["rev-parse", "HEAD"]);
    let output = scratch.path().join("output");
    let mut build = Command::new(env!("CARGO_BIN_EXE_conglobate"));
    build
        .arg("build")
        .arg("--repository")
        .arg(&repository)
        .arg("--commit")
        .arg(&commit)
        .arg("--cache")
        .arg(&cache);
    // Both builders share immutable inputs and assemble identical base chains.
    // Their snapshots, mounts and process namespaces must remain independent.
    let results: Vec<_> = [output.clone(), scratch.path().join("concurrent-output")]
        .into_iter()
        .map(|destination| {
            let mut command = Command::new(build.get_program());
            command
                .args(build.get_args())
                .arg("--output")
                .arg(destination)
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in results {
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let inventory = Inventory::decode(&fs::read(output.join("inventory.cbor")).unwrap()).unwrap();
    assert_eq!(inventory.outputs.scutes.len(), 3);
    assert_eq!(inventory.outputs.scutes[0].origin, "inherited");
    assert_eq!(inventory.outputs.scutes[1].origin, "produced");
    assert_eq!(
        inventory.inputs[0].roles,
        ["carapace-base", "firmware-tools"]
    );
    let root = hex::encode(inventory.outputs.carapace_root.as_ref().unwrap());
    let read_blob = |digest: &Digest| {
        fs::read(
            output
                .join("image.oci/blobs/sha256")
                .join(hex::encode(&digest.digest)),
        )
        .unwrap()
    };
    assert!(read_blob(inventory.outputs.pmi.as_ref().unwrap()).ends_with(root.as_bytes()));
    let dtb = read_blob(inventory.outputs.dtb.as_ref().unwrap());
    let parsed: devtree::Tree<'_> = devtree::Tree::parse(&dtb).unwrap();
    let owned = devtree::OwnedTree::from(&parsed);
    assert_eq!(
        owned
            .find_path("/chosen")
            .unwrap()
            .property("bootargs")
            .unwrap()
            .as_ref(),
        format!("quiet corium={root}\0").as_bytes()
    );
    let manifest: Manifest =
        serde_json::from_slice(&read_blob(&inventory.outputs.manifest)).unwrap();
    assert_eq!(manifest.annotations["example.inherited"], "yes");
    // Published output is independent of the original input cache.
    fs::remove_dir_all(cache).unwrap();
    for layer in manifest.layers {
        assert!(
            output
                .join("image.oci/blobs/sha256")
                .join(&layer.digest_str()[7..])
                .is_file()
        );
    }
    if let Some(pichi) = std::env::var_os("PICHI_TEST_BINARY") {
        import_roundtrip(Path::new(&pichi), scratch.path(), &output, &inventory);
    }
    inherited_firmware(scratch.path(), &output, &inventory);
}

fn inherited_firmware(directory: &Path, parent: &Path, original: &Inventory) {
    let cache = directory.join("inherited-cache");
    fs::create_dir_all(cache.join("blobs/sha256")).unwrap();
    for entry in fs::read_dir(parent.join("image.oci/blobs/sha256")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(
            entry.path(),
            cache.join("blobs/sha256").join(entry.file_name()),
        )
        .unwrap();
    }
    let index = blob(
        &cache,
        &fs::read(parent.join("image.oci/index.json")).unwrap(),
    );
    let repository = directory.join("inherited-repo");
    fs::create_dir_all(repository.join("pichi.build")).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(
        repository.join("pichi.build/refs.lock"),
        format!("version: 1\nimages:\n  parent:\n    index: {index}\n"),
    )
    .unwrap();
    for (name, command) in [("inherited", "printf next > /next"), ("failed", "exit 19")] {
        fs::write(
            repository.join("pichi.build/carapace.yaml"),
            format!("from: parent\nderive:\n  - run: '{command}'\n"),
        )
        .unwrap();
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-qm", name]);
        let commit = git(&repository, &["rev-parse", "HEAD"]);
        let output = directory.join(name);
        let result = Command::new(env!("CARGO_BIN_EXE_conglobate"))
            .arg("build")
            .arg("--repository")
            .arg(&repository)
            .arg("--commit")
            .arg(commit)
            .arg("--cache")
            .arg(&cache)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        if name == "failed" {
            assert!(!result.status.success());
            assert!(!output.exists());
            continue;
        }
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let inherited =
            Inventory::decode(&fs::read(output.join("inventory.cbor")).unwrap()).unwrap();
        assert_eq!(inherited.outputs.pmi, original.outputs.pmi);
        assert_ne!(inherited.outputs.dtb, original.outputs.dtb);
        assert_eq!(
            inherited.outputs.scutes.len(),
            original.outputs.scutes.len() + 1
        );
        let dtb = fs::read(
            output
                .join("image.oci/blobs/sha256")
                .join(hex::encode(&inherited.outputs.dtb.unwrap().digest)),
        )
        .unwrap();
        let parsed: devtree::Tree<'_> = devtree::Tree::parse(&dtb).unwrap();
        let tree = devtree::OwnedTree::from(&parsed);
        assert_eq!(
            tree.find_path("/chosen")
                .unwrap()
                .property("bootargs")
                .unwrap()
                .as_ref(),
            format!(
                "quiet corium={}\0",
                hex::encode(inherited.outputs.carapace_root.unwrap())
            )
            .as_bytes()
        );
    }
}

fn import_roundtrip(pichi: &Path, directory: &Path, output: &Path, inventory: &Inventory) {
    // pichi load may consume its layout's blobs by rename. Import a transport
    // copy, retaining the original as the independent byte-for-byte oracle.
    let transport = directory.join("import-layout");
    fs::create_dir_all(transport.join("blobs/sha256")).unwrap();
    for name in ["oci-layout", "index.json"] {
        fs::copy(output.join("image.oci").join(name), transport.join(name)).unwrap();
    }
    let entries: Vec<_> = fs::read_dir(output.join("image.oci/blobs/sha256"))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!entries.is_empty());
    for entry in entries {
        fs::copy(
            entry.path(),
            transport.join("blobs/sha256").join(entry.file_name()),
        )
        .unwrap();
    }
    let config = directory.join("pichi-config.toml");
    fs::write(
        &config,
        format!(
            "[storage]\ngraphroot = {}\n",
            serde_json::to_string(&directory.join("import-cache")).unwrap()
        ),
    )
    .unwrap();
    let run = |args: &[&std::ffi::OsStr]| {
        let result = Command::new(pichi)
            .args(args)
            .env("PICHI_CONFIG", &config)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    run(&[
        "load".as_ref(),
        transport.as_os_str(),
        "--tag".as_ref(),
        "localhost/conglobate-test:latest".as_ref(),
    ]);
    let saved = directory.join("saved-image");
    run(&[
        "save".as_ref(),
        "localhost/conglobate-test:latest".as_ref(),
        "--output".as_ref(),
        saved.as_os_str(),
    ]);
    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(saved.join("index.json")).unwrap()).unwrap();
    assert_eq!(
        index["manifests"][0]["digest"],
        inventory.outputs.manifest.oci()
    );
    for entry in fs::read_dir(output.join("image.oci/blobs/sha256")).unwrap() {
        let entry = entry.unwrap();
        let saved_blob = saved.join("blobs/sha256").join(entry.file_name());
        // pichi save represents the canonical empty config inline in the
        // unchanged manifest, and need not duplicate its two bytes as a blob.
        if entry.file_name() == ConfigDescriptor::canonical().digest[7..] && !saved_blob.exists() {
            assert_eq!(fs::read(entry.path()).unwrap(), b"{}");
            continue;
        }
        assert_eq!(
            fs::read(entry.path()).unwrap(),
            fs::read(&saved_blob)
                .unwrap_or_else(|error| panic!("{}: {error}", saved_blob.display()))
        );
    }
    eprintln!("pichi load/save round trip verified after removing input cache");
}

#[test]
#[ignore = "requires root, mkfs.ext4, device-mapper and Linux namespaces"]
fn every_stage_combination_has_the_required_output_and_ancestry() {
    let scratch = Scratch::new().unwrap();
    let (cache, index) = seed(scratch.path());
    let root = scratch.path().join("seed-root");
    fs::remove_file(root.join("tool-only")).unwrap();
    let raw = scratch.path().join("independent.raw");
    fs::File::create(&raw)
        .unwrap()
        .set_len(16 * 1024 * 1024)
        .unwrap();
    assert!(
        Command::new("mkfs.ext4")
            .args(["-q", "-F", "-b", "4096", "-d"])
            .arg(&root)
            .arg(&raw)
            .status()
            .unwrap()
            .success()
    );
    let expected = Scute::import(&raw, &scratch.path().join("expected-bootstrap")).unwrap();
    let dtb = devtree::OwnedTree::new(devtree::OwnedNode::new(""))
        .encode()
        .unwrap();
    for combination in 0..8 {
        let bootstrap = combination & 1 != 0;
        let carapace = combination & 2 != 0;
        let firmware = combination & 4 != 0;
        let repository = scratch.path().join(format!("repository-{combination}"));
        fs::create_dir_all(repository.join("pichi.build")).unwrap();
        git(&repository, &["init", "-q"]);
        fs::write(
            repository.join("pichi.build/refs.lock"),
            format!("version: 1\nimages:\n  tool:\n    index: {index}\n"),
        )
        .unwrap();
        fs::write(repository.join("base.dtb"), &dtb).unwrap();
        if bootstrap {
            fs::copy(&raw, repository.join("rootfs.raw")).unwrap();
            fs::write(repository.join("pichi.build/bootstrap.yaml"), "from: tool\nderive:\n  - run: 'printf disposable > /tool-only'\n  - copy:\n      from: rootfs.raw\n      into: /rootfs.raw\ninto: /rootfs.raw\n").unwrap();
        }
        if carapace {
            fs::write(
                repository.join("pichi.build/carapace.yaml"),
                if bootstrap { "" } else { "from: tool\n" },
            )
            .unwrap();
        }
        if firmware {
            let from = if bootstrap || carapace {
                ""
            } else {
                "from: tool\n"
            };
            fs::write(repository.join("pichi.build/firmware.yaml"), format!("{from}derive:\n  - copy:\n      from: base.dtb\n      into: /boot.dtb\n  - run: 'printf firmware > /boot.pmi'\ninto:\n  pmi: /boot.pmi\n  dtb: /boot.dtb\n")).unwrap();
        }
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-qm", "context"]);
        let commit = git(&repository, &["rev-parse", "HEAD"]);
        let output = scratch.path().join(format!("output-{combination}"));
        let result = Command::new(env!("CARGO_BIN_EXE_conglobate"))
            .arg("build")
            .arg("--repository")
            .arg(&repository)
            .arg("--commit")
            .arg(&commit)
            .arg("--cache")
            .arg(&cache)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        if combination == 0 {
            assert!(!result.status.success());
            assert!(!output.exists());
            continue;
        }
        assert!(
            result.status.success(),
            "combination {combination}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let inventory =
            Inventory::decode(&fs::read(output.join("inventory.cbor")).unwrap()).unwrap();
        assert_eq!(
            inventory.outputs.scutes.len(),
            usize::from(bootstrap || carapace)
        );
        assert_eq!(inventory.outputs.pmi.is_some(), firmware);
        assert_eq!(inventory.outputs.dtb.is_some(), firmware);
        let manifest: Manifest = serde_json::from_slice(
            &fs::read(
                output
                    .join("image.oci/blobs/sha256")
                    .join(hex::encode(inventory.outputs.manifest.digest)),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            manifest.annotations.contains_key("example.inherited"),
            carapace && !bootstrap
        );
        if bootstrap {
            assert_eq!(inventory.outputs.scutes[0].origin, "produced");
            assert_eq!(
                inventory.outputs.carapace_root.unwrap().as_ref(),
                expected.root
            );
        }
    }
}

#[test]
#[ignore = "requires root, mkfs.ext4, device-mapper and Linux namespaces"]
fn cancellation_reaps_workers_and_crash_recovery_removes_scratch() {
    use std::io::BufRead;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let scratch = Scratch::new().unwrap();
    let (cache, index) = seed(scratch.path());
    let repository = scratch.path().join("cancel-repo");
    fs::create_dir_all(repository.join("pichi.build")).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(
        repository.join("pichi.build/refs.lock"),
        format!("version: 1\nimages:\n  tool:\n    index: {index}\n"),
    )
    .unwrap();
    fs::write(
        repository.join("pichi.build/carapace.yaml"),
        "from: tool\nderive:\n  - run: 'echo READY; while :; do :; done'\n",
    )
    .unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "cancel fixture"]);
    let commit = git(&repository, &["rev-parse", "HEAD"]);
    let temporary = scratch.path().join("build-scratch");
    fs::create_dir(&temporary).unwrap();
    for signal in [
        nix::sys::signal::Signal::SIGTERM,
        nix::sys::signal::Signal::SIGKILL,
    ] {
        let output = scratch.path().join(format!("cancel-output-{signal}"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_conglobate"))
            .arg("build")
            .arg("--repository")
            .arg(&repository)
            .arg("--commit")
            .arg(&commit)
            .arg("--cache")
            .arg(&cache)
            .arg("--output")
            .arg(&output)
            .env("TMPDIR", &temporary)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut line = String::new();
        let mut diagnostics = std::io::BufReader::new(child.stderr.take().unwrap());
        diagnostics.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "READY");
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(i32::try_from(pid).unwrap()),
            signal,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                child.wait().unwrap();
                panic!("cancelled builder did not exit");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!output.exists());
        let prefix = format!("conglobate-{pid}-");
        loop {
            let devices = fs::read_dir("/sys/block")
                .unwrap()
                .filter_map(Result::ok)
                .filter_map(|entry| fs::read_to_string(entry.path().join("dm/name")).ok())
                .filter(|name| name.starts_with(&prefix))
                .count();
            if devices == 0 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "snapshot leaked after cancellation"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        if signal == nix::sys::signal::Signal::SIGTERM {
            assert_eq!(
                fs::read_dir(&temporary)
                    .unwrap()
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().unwrap().is_dir())
                    .count(),
                0
            );
        }
    }
    fs::write(repository.join("pichi.build/carapace.yaml"), "from: tool\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "recover"]);
    let commit = git(&repository, &["rev-parse", "HEAD"]);
    assert!(
        Command::new(env!("CARGO_BIN_EXE_conglobate"))
            .arg("build")
            .arg("--repository")
            .arg(&repository)
            .arg("--commit")
            .arg(&commit)
            .arg("--cache")
            .arg(&cache)
            .arg("--output")
            .arg(scratch.path().join("recovered"))
            .env("TMPDIR", &temporary)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        fs::read_dir(temporary)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().unwrap().is_dir())
            .count(),
        0
    );

    // An installed executable may be replaced while its old process runs.
    // Workers and the inventory must still identify the running builder bytes.
    fs::write(repository.join("pichi.build/carapace.yaml"), "from: tool\nderive:\n  - run: \"trap 'exit 0' USR1; echo READY; while :; do :; done\"\n  - run: 'printf stable > /stable'\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "replace executable"]);
    let commit = git(&repository, &["rev-parse", "HEAD"]);
    let executable = scratch.path().join("installed-conglobate");
    fs::copy(env!("CARGO_BIN_EXE_conglobate"), &executable).unwrap();
    let expected = Digest::sha256(&fs::read(&executable).unwrap());
    let output = scratch.path().join("replaced-executable-output");
    let mut child = Command::new(&executable)
        .arg("build")
        .arg("--repository")
        .arg(&repository)
        .arg("--commit")
        .arg(commit)
        .arg("--cache")
        .arg(&cache)
        .arg("--output")
        .arg(&output)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut diagnostics = std::io::BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    diagnostics.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "READY");
    fs::remove_file(&executable).unwrap();
    let mut shell_pid = child.id();
    // Builder -> step supervisor -> PID-namespace init -> shell. Use the
    // observer's /proc view to address the shell without exposing host PIDs.
    for _ in 0..3 {
        let children =
            fs::read_to_string(format!("/proc/{shell_pid}/task/{shell_pid}/children")).unwrap();
        let children: Vec<u32> = children
            .split_whitespace()
            .map(|pid| pid.parse().unwrap())
            .collect();
        assert_eq!(children.len(), 1);
        shell_pid = children[0];
    }
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(i32::try_from(shell_pid).unwrap()),
        nix::sys::signal::Signal::SIGUSR1,
    )
    .unwrap();
    assert!(child.wait().unwrap().success());
    let inventory = Inventory::decode(&fs::read(output.join("inventory.cbor")).unwrap()).unwrap();
    assert_eq!(inventory.builder.executable, expected);
}
