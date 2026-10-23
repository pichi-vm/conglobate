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
    fn oci_copy(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("index.json"))?)?;
        let digest = index["manifests"][0]["digest"].as_str().unwrap();
        let copied = Path::new("/work/oras-copy");
        assert!(!copied.exists());
        let result = Command::new("/usr/bin/oras")
            .args(["cp", "--from-oci-layout", "--to-oci-layout"])
            .arg(format!("{}@{digest}", output.display()))
            .arg(format!("{}:verified", copied.display()))
            .output()?;
        assert!(
            result.status.success(),
            "ORAS {}: {}",
            output.display(),
            String::from_utf8_lossy(&result.stderr)
        );
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(copied.join("index.json"))?)?;
        assert_eq!(saved["manifests"][0]["digest"].as_str(), Some(digest));
        let mut count = 0;
        for entry in fs::read_dir(output.join("blobs/sha256"))? {
            let entry = entry?;
            let original = fs::read(entry.path())?;
            assert_eq!(
                hex::encode(Sha256::digest(&original)),
                entry.file_name().to_str().unwrap()
            );
            assert_eq!(
                original,
                fs::read(copied.join("blobs/sha256").join(entry.file_name()))?
            );
            count += 1;
        }
        assert_eq!(count, fs::read_dir(copied.join("blobs/sha256"))?.count());
        fs::remove_dir_all(copied)?;
        println!(
            "PIPELINE_ORAS_SELF_CONTAINED_COPY_PASS {}",
            output.display()
        );
        Ok(())
    }
    fn network_listener(
        count: usize,
    ) -> std::io::Result<std::thread::JoinHandle<std::io::Result<()>>> {
        use std::io::{BufRead, Write};
        let listener = std::net::TcpListener::bind("0.0.0.0:45459")?;
        listener.set_nonblocking(true)?;
        Ok(std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
            for _ in 0..count {
                loop {
                    let remaining = deadline
                        .checked_duration_since(std::time::Instant::now())
                        .ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "CLI network listener",
                            )
                        })?;
                    let mut fds = [rustix::event::PollFd::new(
                        &listener,
                        rustix::event::PollFlags::IN,
                    )];
                    rustix::event::poll(
                        &mut fds,
                        Some(&rustix::event::Timespec::try_from(remaining).unwrap()),
                    )?;
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                            stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                            let mut request = String::new();
                            std::io::BufReader::new(&stream).read_line(&mut request)?;
                            assert_eq!(request, "request\n");
                            stream.write_all(b"reply\n")?;
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(error) => return Err(error),
                    }
                }
            }
            Ok(())
        }))
    }
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
        let formatter_files: Vec<_> = fs::read_to_string("/formatter-runtime-files")?
            .lines()
            .map(str::to_owned)
            .collect();
        fs::write(
            "/work/trusted-formatter.yml",
            serde_json::to_vec(&serde_json::json!({
                "root":"/", "executable":"usr/bin/mke2fs", "files":formatter_files,
            }))?,
        )?;
        fs::write(
            "/work/incomplete-formatter.yml",
            serde_json::to_vec(&serde_json::json!({
                "root":"/", "executable":"usr/bin/mke2fs", "files":["usr/bin/mke2fs"],
            }))?,
        )?;
        for (name, command) in [
            (
                "bootstrap",
                "test ! -e /mnt/bin; echo disposable > /tool-only; echo fresh > /mnt/fresh",
            ),
            ("bootstrap-fail", "echo rejected > /mnt/rejected; exit 37"),
        ] {
            fs::write(
                repo.join(format!("{name}.yml")),
                serde_json::to_vec(&serde_json::json!({
                    "bootstrap": {
                        "from":"example.test/tools:1",
                        "filesystem":{"type":"ext4", "size":"16MiB", "properties":{"label":"cli-root"}},
                        "derive":[{"run":command}, {"copy":{"from":"input", "into":"/mnt/copied"}}],
                    },
                }))?,
            )?;
        }
        // Measured context files install a shell in the fresh output for a later
        // retained RUN. They enter only through COPY, never tool inheritance.
        for directory in ["bin", "lib64"] {
            fs::create_dir_all(repo.join("runtime").join(directory))?;
            for entry in fs::read_dir(Path::new("/image-root").join(directory))? {
                let entry = entry?;
                fs::copy(
                    entry.path(),
                    repo.join("runtime").join(directory).join(entry.file_name()),
                )?;
            }
        }
        for directory in ["dev", "proc", "sys"] {
            fs::create_dir_all(repo.join("runtime").join(directory))?;
            // Git does not retain empty directories. The image explicitly
            // supplies its Linux runtime mountpoints through measured COPY.
            fs::write(repo.join("runtime").join(directory).join(".keep"), b"")?;
        }
        for (name, run) in [
            (
                "bootstrap-layers",
                "test $(cat /fresh) = fresh; test ! -e /tool-only; echo retained > /layered",
            ),
            (
                "bootstrap-layers-fail",
                "echo rejected > /rejected; exit 37",
            ),
        ] {
            let mut recipe: serde_json::Value =
                serde_json::from_slice(&fs::read(repo.join("bootstrap.yml"))?)?;
            recipe["carapace"] = serde_json::json!({"derive":[
                {"copy":{"from":"runtime/", "into":"/"}},
                {"run":run},
                {"copy":{"from":"input", "into":"/layer-copy"}},
            ]});
            fs::write(
                repo.join(format!("{name}.yml")),
                serde_json::to_vec(&recipe)?,
            )?;
        }
        let mut missing: serde_json::Value =
            serde_json::from_slice(&fs::read(repo.join("bootstrap.yml"))?)?;
        missing["carapace"] = serde_json::json!({"derive":[{"run":"exit 0"}]});
        fs::write(
            repo.join("bootstrap-missing-targets.yml"),
            serde_json::to_vec(&missing)?,
        )?;
        let dtb = Self::empty_dtb();
        fs::write(repo.join("firmware.pmi"), b"custom firmware output")?;
        fs::write(repo.join("firmware.dtb"), &dtb)?;
        for (name, layered, explicit, missing) in [
            ("bootstrap-fw-implicit", false, false, false),
            ("bootstrap-fw-explicit", false, true, false),
            ("bootstrap-fw-layered", true, false, false),
            ("bootstrap-fw-layered-explicit", true, true, false),
            ("bootstrap-fw-missing", false, true, true),
        ] {
            let source = if layered {
                "bootstrap-layers.yml"
            } else {
                "bootstrap.yml"
            };
            let mut recipe: serde_json::Value =
                serde_json::from_slice(&fs::read(repo.join(source))?)?;
            if !layered && !explicit {
                recipe["bootstrap"]["derive"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"copy":{"from":"runtime/", "into":"/mnt/"}}));
            }
            let visibility = if explicit {
                "test ! -e /fresh; test ! -e /layered"
            } else if layered {
                "test $(cat /fresh) = fresh; test $(cat /layered) = retained"
            } else {
                "test $(cat /fresh) = fresh"
            };
            recipe["firmware"] = serde_json::json!({
                "derive":[
                    {"copy":{"from":"firmware.dtb", "into":"/out/image.dtb"}},
                    {"run":format!("{visibility}; printf '%s' \"$CARAPACE\" > /out/image.pmi; echo discarded > /firmware-tool-change")},
                ],
                "into":{"pmi":"/out/image.pmi", "dtb":if missing {"/missing.dtb"} else {"/out/image.dtb"}},
            });
            if explicit {
                recipe["firmware"]["from"] = "example.test/tools:1".into();
            }
            fs::write(
                repo.join(format!("{name}.yml")),
                serde_json::to_vec(&recipe)?,
            )?;
        }
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
        for (name, explicit, missing) in [
            ("composed-implicit", false, false),
            ("composed-explicit", true, false),
            ("composed-missing", false, true),
        ] {
            let visibility = if explicit {
                "test ! -e /kept"
            } else {
                "test $(cat /kept) = retained"
            };
            let mut document = serde_json::json!({
                "carapace":{"from":"example.test/tools:1","derive":[{"run":"echo retained > /kept"}]},
                "firmware":{"derive":[
                    {"copy":{"from":"firmware.dtb","into":"/out/image.dtb"}},
                    {"run":format!("{visibility}; test \"$CARAPACE\" != {}; printf '%s' \"$CARAPACE\" > /out/image.pmi; echo disposable > /firmware-tool-change", hex::encode(base_root))}
                ],"into":{"pmi":"/out/image.pmi","dtb":if missing {"/missing.dtb"} else {"/out/image.dtb"}}}
            });
            if explicit {
                document["firmware"]["from"] = "example.test/tools:1".into();
            }
            fs::write(
                repo.join(format!("{name}.yml")),
                serde_json::to_vec(&document)?,
            )?;
        }
        let mut network_recipe: serde_json::Value =
            serde_json::from_slice(&fs::read(repo.join("bootstrap-fw-layered-explicit.yml"))?)?;
        let mut network_count = 0;
        for stage in ["bootstrap", "carapace", "firmware"] {
            for directive in network_recipe[stage]["derive"].as_array_mut().unwrap() {
                if let Some(run) = directive
                    .get("run")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                {
                    directive["network"] = true.into();
                    directive["run"] = format!("echo PIPELINE_COMMAND_LOG; test \"$(cat /etc/resolv.conf)\" = 'nameserver 10.0.0.1'; exec 3<>/dev/tcp/10.0.0.1/45459; echo request >&3; IFS= read -r reply <&3; test \"$reply\" = reply; exec 3>&-; {run}").into();
                    network_count += 1;
                }
            }
        }
        fs::write(
            repo.join("network-pipeline.yml"),
            serde_json::to_vec(&network_recipe)?,
        )?;
        fs::write(
            repo.join("network-fail.yml"),
            serde_json::to_vec(&serde_json::json!({
                "carapace": {"from":"example.test/tools:1", "derive":[{
                    "network":true,
                    "run":"test -r /etc/resolv.conf; echo NETWORK_FAILURE_COMMAND_ENTERED; echo rejected > /rejected; exit 37"
                }]}
            }))?,
        )?;
        fs::write("/work/network-resolver", b"nameserver 10.0.0.1\n")?;
        for module in ["veth", "nf_tables", "nft_chain_nat", "nft_masq", "nft_ct"] {
            assert!(Command::new("modprobe").arg(module).status()?.success());
        }
        let forwarding4 = fs::read_to_string("/proc/sys/net/ipv4/ip_forward")?;
        let forwarding6 = fs::read_to_string("/proc/sys/net/ipv6/conf/all/forwarding")?;
        // Disposable fixture VM with no external NIC. Production only reads these.
        fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
        fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
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
            (
                "network-fail.yml",
                "/work/network-no-forwarding4-output",
                false,
            ),
            (
                "network-fail.yml",
                "/work/network-no-forwarding6-output",
                false,
            ),
            (
                "network-fail.yml",
                "/work/network-failed-command-output",
                false,
            ),
            (
                "network-pipeline.yml",
                "/work/network-pipeline-output",
                true,
            ),
            (
                "network-pipeline.yml",
                "/work/network-missing-resolver-output",
                false,
            ),
            ("build.yml", "/work/derived-output", true),
            ("bootstrap.yml", "/work/bootstrap-output", true),
            (
                "bootstrap-fw-implicit.yml",
                "/work/bootstrap-fw-implicit-output",
                true,
            ),
            (
                "bootstrap-fw-explicit.yml",
                "/work/bootstrap-fw-explicit-output",
                true,
            ),
            (
                "bootstrap-fw-layered.yml",
                "/work/bootstrap-fw-layered-output",
                true,
            ),
            (
                "bootstrap-fw-layered-explicit.yml",
                "/work/bootstrap-fw-layered-explicit-output",
                true,
            ),
            (
                "bootstrap-fw-missing.yml",
                "/work/bootstrap-fw-missing-output",
                false,
            ),
            (
                "bootstrap-missing-targets.yml",
                "/work/bootstrap-missing-targets-output",
                false,
            ),
            (
                "bootstrap-layers.yml",
                "/work/bootstrap-layers-output",
                true,
            ),
            (
                "bootstrap-layers-fail.yml",
                "/work/bootstrap-layers-fail-output",
                false,
            ),
            ("bootstrap.yml", "/work/bootstrap-no-runtime-output", false),
            (
                "bootstrap.yml",
                "/work/bootstrap-incomplete-runtime-output",
                false,
            ),
            ("bootstrap-fail.yml", "/work/bootstrap-fail-output", false),
            ("fail.yml", "/work/rejected-output", false),
            ("firmware-only.yml", "/work/firmware-only-output", true),
            ("firmware-base.yml", "/work/firmware-base-output", true),
            (
                "composed-implicit.yml",
                "/work/composed-implicit-output",
                true,
            ),
            (
                "composed-explicit.yml",
                "/work/composed-explicit-output",
                true,
            ),
            (
                "composed-missing.yml",
                "/work/composed-missing-output",
                false,
            ),
            (
                "firmware-missing.yml",
                "/work/firmware-missing-output",
                false,
            ),
        ] {
            let mut command = Command::new("/usr/bin/conglobate");
            command.args([
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
            ]);
            if output != "/work/bootstrap-no-runtime-output" {
                command.args([
                    "--formatter-runtime",
                    if output == "/work/bootstrap-incomplete-runtime-output" {
                        "/work/incomplete-formatter.yml"
                    } else {
                        "/work/trusted-formatter.yml"
                    },
                ]);
            }
            if recipe == "network-fail.yml" {
                command.args(["--resolv-conf", "/work/network-resolver"]);
                if output == "/work/network-no-forwarding4-output" {
                    fs::write("/proc/sys/net/ipv4/ip_forward", "0")?;
                }
                if output == "/work/network-no-forwarding6-output" {
                    fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "0")?;
                }
            }
            let listener = if output == "/work/network-pipeline-output" {
                command.args(["--resolv-conf", "/work/network-resolver"]);
                Some(Self::network_listener(network_count)?)
            } else {
                None
            };
            println!("PIPELINE_CLI_CASE {recipe} -> {output}");
            let result = command.output()?;
            assert!(
                result.stdout.is_empty(),
                "build logs escaped to stdout: {recipe}"
            );
            assert_eq!(
                result.status.success(),
                success,
                "{recipe} -> {output}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            if recipe == "network-fail.yml" {
                let stderr = String::from_utf8_lossy(&result.stderr);
                let entered = stderr.contains("NETWORK_FAILURE_COMMAND_ENTERED");
                if output == "/work/network-failed-command-output" {
                    assert!(entered, "failed command was not admitted: {stderr}");
                    assert!(
                        stderr.contains("37"),
                        "missing primary command status: {stderr}"
                    );
                    println!("PIPELINE_CLI_NETWORK_NONZERO_NO_PUBLICATION_PASS");
                } else {
                    assert!(!entered, "command ran without forwarding");
                    assert!(
                        stderr.contains("require environment forwarding"),
                        "wrong failure: {stderr}"
                    );
                    println!("PIPELINE_CLI_NETWORK_FORWARDING_PREFLIGHT_CLEANUP_PASS");
                }
                assert!(!Path::new("/sys/class/net/cgb0h").exists());
                assert!(
                    !Command::new("nft")
                        .args(["list", "table", "inet", "conglobate_cgb0h"])
                        .output()?
                        .status
                        .success()
                );
                fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
                fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
            }
            if let Some(listener) = listener {
                assert_eq!(
                    String::from_utf8_lossy(&result.stderr)
                        .matches("PIPELINE_COMMAND_LOG")
                        .count(),
                    network_count
                );
                println!("PIPELINE_CLI_COMMAND_LOGS_STDERR_PASS");
                listener
                    .join()
                    .expect("network fixture listener panicked")?;
                let inventory = fs::read(Path::new(output).join("inventory.cbor"))?;
                conglobate::inventory::Inventory::decode(&inventory)?;
                println!("CLI_NETWORK_INVENTORY_HEX {}", hex::encode(&inventory));
                let entries = Field(&inventory).get(&["execution", "network_steps"])?;
                let mut decoder = minicbor::Decoder::new(entries.0);
                assert_eq!(decoder.array()?, Some(network_count as u64));
                let mut stages = Vec::new();
                for _ in 0..network_count {
                    let start = decoder.position();
                    decoder.skip()?;
                    let entry = Field(&entries.0[start..decoder.position()]);
                    stages.push(entry.get(&["stage"])?.text()?.to_owned());
                }
                assert!(stages.iter().any(|s| s == "bootstrap"));
                assert!(stages.iter().any(|s| s == "carapace"));
                assert!(stages.iter().any(|s| s == "firmware"));
                assert!(!Path::new("/sys/class/net/cgb0h").exists());
                assert!(
                    !Command::new("nft")
                        .args(["list", "table", "inet", "conglobate_cgb0h"])
                        .output()?
                        .status
                        .success()
                );
                println!("PIPELINE_CLI_NETWORK_ALL_STAGES_AND_INVENTORY_PASS");
            }
            if output == "/work/network-missing-resolver-output" {
                assert!(String::from_utf8_lossy(&result.stderr).contains("--resolv-conf"));
                println!("PIPELINE_CLI_NETWORK_MISSING_RESOLVER_REJECTED_PASS");
            }
            if recipe == "bootstrap-missing-targets.yml" {
                assert!(
                    String::from_utf8_lossy(&result.stderr).contains("command runtime target /dev")
                );
            }
            assert_eq!(Path::new(output).exists(), success);
            if success {
                Self::oci_copy(Path::new(output))?;
            }

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
        fs::write("/proc/sys/net/ipv4/ip_forward", forwarding4)?;
        fs::write("/proc/sys/net/ipv6/conf/all/forwarding", forwarding6)?;
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
        for (name, count) in [
            ("derived", 4),
            ("bootstrap", 1),
            ("bootstrap-layers", 4),
            ("bootstrap-fw-implicit", 1),
            ("bootstrap-fw-explicit", 1),
            ("bootstrap-fw-layered", 4),
            ("bootstrap-fw-layered-explicit", 4),
            ("composed-implicit", 2),
            ("composed-explicit", 2),
        ] {
            let output = Path::new("/work").join(format!("{name}-output"));
            // Install only the index lookup blob for the cache reader, after checking
            // the published inventory. Production output contains no such extra blob.
            let index = fs::read(output.join("index.json"))?;
            let digest = hex::encode(Sha256::digest(&index));
            fs::write(output.join("blobs/sha256").join(&digest), &index)?;
            let cache = Cache::open(&output)?;
            let image = cache.resolve(format!("sha256:{digest}").parse()?)?;
            cache.verify_references(&image)?;
            let chain = cache.verify_chain(&image)?.ok_or("missing derived chain")?;
            assert_eq!(chain.scutes().len(), count);
            if count == 2 {
                let manifest: serde_json::Value =
                    serde_json::from_slice(image.manifest_bytes().bytes())?;
                let pmi = manifest["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|l| l["mediaType"] == "application/vnd.pichi.pmi.v1")
                    .unwrap();
                let digest = pmi["digest"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("sha256:")
                    .unwrap();
                assert_eq!(
                    fs::read(output.join("blobs/sha256").join(digest))?,
                    hex::encode(chain.root()).as_bytes()
                );
                assert_ne!(chain.root(), &base_root);
                let inventory = fs::read(output.join("inventory.cbor"))?;
                conglobate::inventory::Inventory::decode(&inventory)?;
                assert_eq!(
                    Field(&inventory)
                        .get(&["outputs", "carapace_root"])?
                        .bytes()?,
                    chain.root()
                );
                let entries = Field(&inventory).get(&["outputs", "scutes"])?;
                let mut decoder = minicbor::Decoder::new(entries.0);
                assert_eq!(decoder.array()?, Some(2));
                for origin in ["inherited", "produced"] {
                    let start = decoder.position();
                    decoder.skip()?;
                    assert_eq!(
                        Field(&entries.0[start..decoder.position()])
                            .get(&["origin"])?
                            .text()?,
                        origin
                    );
                }
            }
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
            if name.starts_with("bootstrap") {
                assert_eq!(fs::read(directory.join("fresh"))?, b"fresh\n");
                assert_eq!(fs::read(directory.join("copied"))?, b"committed copy\n");
                assert_eq!(
                    directory.join("bin").exists(),
                    name != "bootstrap" && name != "bootstrap-fw-explicit"
                );
                if name == "bootstrap-layers" || name.starts_with("bootstrap-fw-layered") {
                    assert_eq!(fs::read(directory.join("layered"))?, b"retained\n");
                    assert_eq!(fs::read(directory.join("layer-copy"))?, b"committed copy\n");
                }
                assert!(!directory.join("tool-only").exists());
                let firmware = name.starts_with("bootstrap-fw-");
                assert_eq!(
                    image.manifest().layers.len(),
                    count + if firmware { 2 } else { 0 }
                );
                assert!(!directory.join("firmware-tool-change").exists());
                assert!(!directory.join("out/image.pmi").exists());
                if firmware {
                    let manifest: serde_json::Value =
                        serde_json::from_slice(image.manifest_bytes().bytes())?;
                    let pmi = manifest["layers"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|layer| layer["mediaType"] == "application/vnd.pichi.pmi.v1")
                        .unwrap();
                    let digest = pmi["digest"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("sha256:")
                        .unwrap();
                    assert_eq!(
                        fs::read(output.join("blobs/sha256").join(digest))?,
                        hex::encode(chain.root()).as_bytes()
                    );
                    assert_ne!(chain.root(), &base_root);
                }
                let bytes = fs::read(output.join("inventory.cbor"))?;
                conglobate::inventory::Inventory::decode(&bytes)?;
                let value: ciborium::Value = ciborium::from_reader(bytes.as_slice())?;
                let value = serde_json::to_value(value)?;
                assert_eq!(value["outputs"]["scutes"].as_array().unwrap().len(), count);
                for scute in value["outputs"]["scutes"].as_array().unwrap() {
                    assert_eq!(scute["origin"], "produced");
                }
                assert_eq!(
                    value["execution"]["formatters"].as_array().unwrap().len(),
                    1
                );
                assert_eq!(
                    value["execution"]["formatters"][0]["filesystem"]["label"],
                    "cli-root"
                );
                assert_eq!(
                    value["execution"]["formatters"][0]["filesystem"]["size_bytes"],
                    16 * 1024 * 1024
                );
            } else if count == 4 {
                assert_eq!(fs::read(directory.join("derived"))?, b"final\n");
                assert_eq!(fs::read(directory.join("copied"))?, b"committed copy\n");
            } else {
                assert_eq!(fs::read(directory.join("kept"))?, b"retained\n");
                assert!(!directory.join("firmware-tool-change").exists());
                assert!(!directory.join("out/image.pmi").exists());
            }
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
        }
        println!("PIPELINE_CLI_BOOTSTRAP_PASS");
        println!("PIPELINE_CLI_BOOTSTRAP_LAYERS_PASS");
        println!("PIPELINE_CLI_BOOTSTRAP_FIRMWARE_PASS");
        println!("PIPELINE_CLI_COMPOSED_FIRMWARE_PASS");
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
        assert!(unsupported.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&unsupported.stderr).contains("MUST_NOT_RUN"));
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
