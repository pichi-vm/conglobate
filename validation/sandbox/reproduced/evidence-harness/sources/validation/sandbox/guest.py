#!/usr/bin/env python3
"""Package and run disposable sandbox probes. No host mounts or root required."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sysconfig

HERE = pathlib.Path(__file__).resolve().parent
CASES = {
    "pressure": {"init": "pressure/init", "worker": "pressure/worker"},
    "acl": {"init": "acl/init", "acl-worker": "acl/probe-worker",
            "hang-worker": "hang-worker"},
    "cancellation": {"init": "cancellation-init", "hang-worker": "hang-worker"},
    "concurrent": {"init": "concurrent/init", "worker": "concurrent/worker"},
    "faults": {"init": "faults/init", "fault-worker": "faults/fault-worker"},
    "storage": {"init": "storage/init", "worker": "storage/worker"},
    "keys": {"init": "keys/init", "probe.py": "keys/probe.py"},
    "loader": {p: f"loader/{p}" for p in ("init", "launch", "invoke.py", "check")},
    "entry": {"init": "entry/init", "launch": "entry/launch", "check": "entry/check"},
    "guardian": {"init": "guardian/init", "guardian.py": "guardian/guardian.py",
                 "supervisor.py": "guardian/supervisor.py",
                 "launch": "lifecycle/launch", "probe.py": "lifecycle/probe.py"},
    "lifecycle": {p: f"lifecycle/{p}" for p in ("init", "launch", "probe.py", "driver.py")},
    "policy": {p: f"policy/{p}" for p in ("init", "launch", "probe.py", "driver.py")},
    "copy": {p: f"copy/{p}" for p in ("init", "worker", "launch", "command")},
    "export": {p: f"copy/{p}" for p in ("init", "worker", "launch", "command")},
    "chain-publication": {"init": "chain-publication/init", "chain-worker": "chain-command/worker",
                          "command-launch": "chain-command/launch", "command": "chain-command/command", "verify": "chain/verify"},
    "chain-publication-failure": {"init": "chain-publication/failure-init", "chain-worker": "chain-command/worker",
                                  "command-launch": "chain-command/launch", "command": "chain-command/command", "verify": "chain/verify"},
    "chain-long": {"init": "chain-long/init", "chain-worker": "chain-command/worker",
                   "command-launch": "chain-command/launch", "command": "chain-long/command",
                   "verify": "chain-long/verify"},
    "chain-command-failure": {"init": "chain-command/failure-init", "chain-worker": "chain-command/worker",
                              "command-launch": "chain-command/launch", "command": "chain-command/command",
                              "verify": "chain/verify"},
    "chain-command": {"init": "chain-command/init", "chain-worker": "chain-command/worker",
                      "command-launch": "chain-command/launch", "command": "chain-command/command",
                      "verify": "chain/verify"},
    "chain": {"init": "chain/init", "chain-worker": "chain/worker", "verify": "chain/verify"},
    "publication": {p: f"publication/{p}" for p in ("init", "worker")},
    "finalize": {p: f"finalize/{p}" for p in ("init", "worker")},
    "metadata": {p: f"metadata/{p}" for p in ("init", "worker", "probe.py")},
    "acl-fix": {p: f"acl-fix/{p}" for p in ("init", "worker", "probe.py")},
    "network-policy": {"init": "network-policy/init", "launch": "network-policy/launch",
                       "client.py": "network-policy/client.py",
                       "listener.py": "network-policy/listener.py",
                       "server.py": "network-policy/server.py",
                       "stub.py": "network-policy/stub.py"},
    "network": {p: f"network/{p}" for p in
                ("init", "server.py", "client.py", "resources.py", "ipc.py")},
}
MARKERS = {
    "pressure": ["PRESSURE_QUEUED abort 32", "PRESSURE_QUEUED clients-first 32",
                 "PRESSURE_CLIENTS_REAPED abort", "PRESSURE_CLIENTS_REAPED clients-first",
                 "PRESSURE_WORKER_REAPED_UNMOUNTED abort", "PRESSURE_WORKER_REAPED_UNMOUNTED clients-first",
                 "PRESSURE_CLEANUP_PASS abort", "PRESSURE_CLEANUP_PASS clients-first",
                 "PRESSURE_ABORT_ERRORS_VERIFIED", "PRESSURE_NEW_REQUEST_REJECTED",
                 "PRESSURE_MATRIX_PASS"],
    "acl": ["ACL_ALLOW_FAIL", "ACL_AFTER_CHMOD_ALLOW_PASS", "ACL_INHERITANCE_FAIL",
            "CONNECTION_RELEASED_PASS"],
    "cancellation": ["HUNG_REQUEST_ABORTED_PASS", "CONNECTION_RELEASED_PASS"],
    "concurrent": ["TWO_CONNECTIONS_ACTIVE_PASS",
                   "CONCURRENT_BUILD_SURVIVES_PASS", "CONCURRENT_CLEANUP_PASS"],
    "faults": ["FAULT_DAEMON_STATUS 0", "FAULT_DAEMON_STATUS 134",
               "Input/output error", "FAULT_PROBE_COMPLETE"],
    "storage": ["SNAPSHOT_EXHAUSTION_DETECTED_PASS", "SNAPSHOT_BASE_UNCHANGED_PASS",
                "PERMISSIONS_ALONE_DO_NOT_REVOKE_WRITERS_CONFIRMED",
                "SEAL_DESCENDANTS_REAPED_PASS", "SEAL_WORKER_EXITED_PASS",
                "QUIESCENT_IMAGE_HASH_STABLE_PASS",
                "MALFORMED_IMAGE_REJECTED magic", "MALFORMED_IMAGE_REJECTED truncated"],
    "network-policy": ["NETWORK_POLICY_PASS", "RESOLVER_BIND_RESTORED_PASS regular",
                       "LOOPBACK_STUB_UNREACHABLE_CONFIRMED", "DNS_SEARCH_DOMAIN_PASS",
                       "BUILDER_ENDPOINT_POSITIVE_CONTROL_PASS",
                       "RESOLVER_BIND_RESTORED_PASS symlink",
                       "NETWORK_RESTRICTED_CLIENT_PASS",
                       "UNSOLICITED_INBOUND_BLOCKED_PASS",
                       "IPV6_UNSOLICITED_INBOUND_BLOCKED_PASS"],
    "network": ["NETWORK_VALIDATION_PASS", "RESOURCE_VALIDATION_PASS",
                "SYSV_IPC_UTS_PRIVATE_SHM_PASS", "SUPERVISOR_IPC_UTS_UNCHANGED_PASS",
                "IPV6_NAT_AND_AAAA_DNS_PASS", "IPV6_BUILDER_ENDPOINT_BLOCKED_PASS",
                "IPV6_UNSOLICITED_INBOUND_BLOCKED_PASS"],
    "keys": ["NEW_SESSION_KEYRING_BLOCKS_SUPERVISOR_KEY_PASS",
             "FRESH_SESSION_SAME_UID_WORKER_KEY_VISIBLE",
             "FRESH_SESSION_DIFFERENT_UID_WORKER_KEY_DENIED", "WORKER_KEY_UID_MATRIX_PASS"],
    "loader": ["HELPER_LOADER_ENV_NEGATIVE_CONTROL_CONFIRMED",
               "HELPER_LAUNCH_ENV_SANITIZED_PASS", "IMAGE_ENV_AND_FDS_CLEAN_PASS",
               "LOADER_MATRIX_PASS"],
    "entry": ["ENTRY_CANARIES_VISIBLE_BEFORE_CLEANUP",
              "ENTRY_CANARIES_INACCESSIBLE_AFTER_CLEANUP_PASS",
              "ENTRY_ORDERING_VALIDATION_PASS"],
    "guardian": ["SUPERVISOR_DEATH_ALONE_LEAVES_WRITER",
                 "GUARDIAN_CLEANUP_PASS prelaunch",
                 "GUARDIAN_CLEANUP_PASS after-userns",
                 "GUARDIAN_CLEANUP_PASS command", "GUARDIAN_MATRIX_PASS"],
    "lifecycle": ["LAUNCHER_DEATH_WITHOUT_CHILD_KILL_LEAVES_WRITER",
                  "KILL_CHILD_WITH_ID_CHANGE_LEAVES_WRITER",
                  "LIFECYCLE_CANCEL_PASS after-userns",
                  "LIFECYCLE_CANCEL_PASS after-root-bind",
                  "LIFECYCLE_CANCEL_PASS after-proc",
                  "LIFECYCLE_CANCEL_PASS before-entry",
                  "LIFECYCLE_CANCEL_PASS command", "LIFECYCLE_MATRIX_PASS"],
    "policy": ["NAMESPACE_ONLY_REAL_KEY_READ_PASS", "POLICY_REAL_KEY_READ_DENIED_PASS",
               "SUPERVISOR_KEY_UNCHANGED_PASS", "NAMESPACE_ONLY_INHERITED_IPC_AND_PATH_VISIBLE",
               "SUPERVISOR_ABSTRACT_SOCKET_POSITIVE_CONTROL", "PRIVATE_NETWORK_ABSTRACT_SOCKET_HIDDEN",
               "SUPERVISOR_IPC_HANDLES_UNCHANGED_PASS", "POLICY_INHERITED_IPC_HANDLES_CLOSED_PASS",
               "POLICY_IDENTITY_ENV_FDS_PASS",
               "POLICY_NAMESPACE_MOUNT_PROCESS_KEY_DENIALS_PASS",
               "POLICY_RAW_SOCKET_DENIALS_PASS",
               "POLICY_SOCKET_ALLOWLIST_AND_DISABLED_NETWORK_PASS",
               "POLICY_PRIVATE_DEVPTS_AND_NAMESPACE_HANDLE_PASS",
               "POLICY_NORMAL_FILE_FORK_THREAD_PASS", "POLICY_STEP_PASS 1",
               "POLICY_STEP_PASS 2", "POLICY_GUEST_PASS"],
    "copy": ["COPY_COMMAND_CONTEXT_INACCESSIBLE_PASS", "COPY_NO_WRITERS_BARRIER_PASS",
             "COPY_REAL_IMPLEMENTATION_METADATA_AND_PATHS_PASS",
             "COPY_FIFO_ACCOUNT_REJECTED_PASS", "COPY_KERNEL_REMOUNT_METADATA_PASS",
             "COPY_WORKER_ISOLATION_VALIDATION_PASS"],
    "export": ["COPY_NO_WRITERS_BARRIER_PASS", "EXPORT_IMPLEMENTATION_TESTS_PASS",
               "test result: ok. 6 passed", "COPY_KERNEL_REMOUNT_METADATA_PASS",
               "COPY_WORKER_ISOLATION_VALIDATION_PASS"],
    "chain-publication": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_RESTRICTED_COMMAND_PASS",
                          "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE", "FUSE_DERIVED_LAYER_SEALED",
                          "DERIVED_KERNEL_FILESYSTEM_PASS", "BASE_SCUTE_UNCHANGED_PASS",
                          "CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS", "CHAIN_PUBLICATION_MATRIX_PASS"],
    "chain-publication-failure": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "worker failed: exit status: 7",
                                  "CHAIN_COMMAND_FAILURE_NO_SEAL_CLEANUP_PASS"],
    "chain-long": ["CHAIN_LONG_MATRIX_PASS", "CHAIN_ROUND_SEALED 3",
                   "CHAIN_COMMAND_ROUND 3", "THREE_ROUNDS_KERNEL_STATE_PASS",
                   "MISSING_MIDDLE_LAYER_REJECTED", "DUPLICATE_MIDDLE_LAYER_REJECTED",
                   "DERIVED_CHAIN_RESTAGED", "BASE_SCUTE_UNCHANGED_PASS",
                   "POST_STAGE_CORRUPTION_REJECTED", "POST_STAGE_RESTORATION_READ_PASS"],
    "chain-command-failure": ["CHAIN_COMMAND_FAILURE_NO_SEAL_CLEANUP_PASS",
                              "worker failed: exit status: 7"],
    "chain-command": ["CHAIN_COMMAND_MATRIX_PASS", "CHAIN_RESTRICTED_COMMAND_PASS",
                      "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE",
                      "FUSE_DERIVED_LAYER_SEALED", "DERIVED_CHAIN_RESTAGED",
                      "DERIVED_KERNEL_FILESYSTEM_PASS", "BASE_SCUTE_UNCHANGED_PASS",
                      "POST_STAGE_CORRUPTION_REJECTED", "POST_STAGE_RESTORATION_READ_PASS"],
    "chain": ["BASE_WRONG_ROOT_REJECTED", "BASE_CORRUPT_BLOB_REJECTED",
              "FUSE_DERIVED_LAYER_SEALED", "DERIVED_CHAIN_RESTAGED",
              "DERIVED_KERNEL_FILESYSTEM_PASS", "BASE_SCUTE_UNCHANGED_PASS",
              "POST_STAGE_CORRUPTION_REJECTED",
              "POST_STAGE_RESTORATION_READ_PASS",
              "CHAIN_MATRIX_PASS"],
    "publication": ["PUBLICATION_ACCEPTED patched-clean",
                    "PUBLICATION_ACCEPTED baseline-close",
                    "PUBLICATION_REJECTED patched-close",
                    "PUBLICATION_REJECTED patched-error",
                    "PUBLICATION_REJECTED patched-recovered",
                    "ARTIFACT_ENOSPC_CONFIRMED", "PUBLICATION_REJECTED_ENOSPC",
                    "PUBLICATION_COPY_ENOSPC_CLEANUP_PASS",
                    "REAL_SCUTE_INVENTORY_PUBLICATION_PASS",
                    "PUBLICATION_MATRIX_COMPLETE"],
    "finalize": ["FINALIZE_STATUS baseline-clean 0", "FINALIZE_STATUS patched-clean 0",
                 "FINALIZE_STATUS baseline-close 0", "FINALIZE_STATUS patched-close 32",
                 "FINALIZE_STATUS baseline-error 0", "FINALIZE_STATUS patched-error 32",
                 "FINALIZE_STATUS patched-recovered 32", "DIAGNOSTIC_CLOSE_ONLY_EIO",
                 "FINALIZATION_STATUS_MATRIX_PASS"],
    "metadata": ["METADATA_MATRIX_COMPLETE", "METADATA_KERNEL_REMOUNT_MATCH_PASS",
                 "METADATA_REFERENCE_COMPARED mapped without-metadata-csum"],
    "acl-fix": ["ACL_REFERENCE_MATCH_PASS initial metadata-csum",
                "ACL_REFERENCE_MATCH_PASS mapped metadata-csum",
                "ACL_REFERENCE_MATCH_PASS initial without-metadata-csum",
                "ACL_REFERENCE_MATCH_PASS mapped without-metadata-csum",
                "ACL_COMMON_SEMANTICS_MATRIX_PASS",
                "ACL_NONROOT_FSETID_SGID_CLEARED_LIMITATION"],
}


def capture_sources(case):
    """Record local compile inputs and fixture sources, excluding build outputs."""
    repository = HERE.parents[1]
    paths = {pathlib.Path(__file__).resolve()}
    paths.update(HERE / relative for relative in CASES[case].values())
    for project in (repository, HERE / "entry/helper", HERE / "copy/helper",
                    HERE / "publication/helper"):
        paths.update((project / "src").rglob("*.rs"))
        paths.update(path for path in (project / "Cargo.toml", project / "Cargo.lock",
                                      project / "build.rs") if path.is_file())
        if (project / ".cargo").is_dir():
            paths.update(path for path in (project / ".cargo").rglob("*") if path.is_file())
    return {str(path.relative_to(repository)): path.read_bytes() for path in sorted(paths)}


def validate_guest_exit(result):
    exits = [line for line in result.splitlines() if line.startswith("GUEST_EXIT")]
    if exits != ["GUEST_EXIT 0"]:
        raise RuntimeError(f"Expected one successful guest exit, got {exits!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("case", choices=CASES)
    parser.add_argument("--kernel-version", required=True)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--worker-build", type=pathlib.Path,
                        help="finalize fixture's baseline/patched worker build directory")
    args = parser.parse_args()
    sources = capture_sources(args.case)
    out = args.out.resolve()
    out.mkdir()  # Refuse to overwrite or reuse an existing fixture.
    root = out / "root"
    root.mkdir()
    for name in ("usr/bin", "usr/sbin", "usr/lib", "usr/lib64", "etc", "proc",
                 "sys", "dev", "tmp", "work", "run/netns", "var/run/netns"):
        (root / name).mkdir(parents=True, exist_ok=True)
    for link, target in (("bin", "usr/bin"), ("sbin", "usr/sbin"),
                         ("lib", "usr/lib"), ("lib64", "usr/lib64")):
        (root / link).symlink_to(target)
    copied = set()

    def copy(path):
        path = pathlib.Path(path)
        if path in copied:
            return
        copied.add(path)
        dest = root / str(path).lstrip("/")
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, dest)

    def libraries(path):
        result = subprocess.run(["ldd", str(path)], capture_output=True, text=True)
        # ldd fails for scripts and static executables; neither needs shared libs.
        for lib in re.findall(r"(/[^\s()]+)", result.stdout):
            if pathlib.Path(lib).is_file():
                copy(lib)
        if "not found" in result.stdout:
            raise RuntimeError(f"Missing library dependency for {path}")

    tools = ["bash", "mount", "umount", "unshare", "fuse2fs", "fusermount3",
             "mkfs.ext4", "e2fsck", "truncate", "mkdir", "chmod", "chown", "touch",
             "stat", "setpriv", "setfacl", "getfacl", "cat", "sleep", "mountpoint",
             "modprobe", "rm", "sync", "losetup", "dmsetup", "dd", "sha256sum",
             "cp", "timeout", "pivot_root", "ln", "mknod", "mv"]
    if args.case in ("chain-publication", "chain-publication-failure"):
        tools += ["git"]
    if args.case in ("network", "network-policy"):
        tools += ["ip", "nft", "python3", "sysctl"]
    elif args.case in ("keys", "policy", "loader", "lifecycle", "guardian", "acl-fix", "metadata"):
        tools += ["python3"]
    for tool in tools:
        path = shutil.which(tool)
        if path is None:
            raise RuntimeError(f"Required host executable missing: {tool}")
        copy(path)
        libraries(path)
    if args.case == "loader":
        binary = root / "loader-canary.so"
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                        str(HERE / "loader/canary.c"), "-o", str(binary)], check=True)
        libraries(binary)
    if args.case in ("finalize", "publication", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure"):
        if args.worker_build is None:
            raise RuntimeError("finalize requires --worker-build from finalize/build.py")
        for name in ("baseline", "baseline-close", "patched", "patched-close"):
            binary = args.worker_build / name
            shutil.copy2(binary, root / name)
            libraries(binary)
    if args.case in ("publication", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure"):
        helper = HERE / "publication/helper"
        target = HERE / "copy/helper/target"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(helper / "Cargo.toml"), "--target-dir", str(target)], check=True)
        binary = target / ("debug/chain" if args.case in ("chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure") else "debug/conglobate-publication-probe")
        shutil.copy2(binary, root / "publication-helper")
        libraries(binary)
    if args.case in ("acl-fix", "metadata"):
        if args.worker_build is None:
            raise RuntimeError("acl-fix requires --worker-build from acl-fix/build.py")
        binary = args.worker_build / "acl-patched"
        shutil.copy2(binary, root / "acl-patched")
        libraries(binary)
    if args.case in ("entry", "loader", "policy", "lifecycle", "guardian", "copy", "export", "network-policy", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure"):
        helper = HERE / "entry/helper"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(helper / "Cargo.toml")], check=True)
        binary = helper / "target/debug/conglobate-entry-probe"
        shutil.copy2(binary, root / "entry-helper")
        libraries(binary)
    if args.case in ("copy", "export"):
        helper = HERE / "copy/helper"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(helper / "Cargo.toml")], check=True)
        binary = helper / "target/debug/conglobate-copy-probe"
        shutil.copy2(binary, root / "copy-helper")
        libraries(binary)
    if args.case == "export":
        result = subprocess.run(
            ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json",
             "--manifest-path", str(HERE.parents[1] / "Cargo.toml")],
            capture_output=True, text=True, check=True)
        binaries = [
            entry["executable"] for line in result.stdout.splitlines()
            if (entry := json.loads(line)).get("reason") == "compiler-artifact"
            and entry.get("executable")
            and entry["target"]["name"] == "conglobate"
            and entry["target"]["kind"] == ["lib"]
        ]
        if len(binaries) != 1:
            raise RuntimeError(f"Expected one conglobate library test binary, got {binaries}")
        shutil.copy2(binaries[0], root / "export-tests")
        libraries(binaries[0])
    if args.case in ("network", "network-policy", "keys", "policy", "loader", "lifecycle", "guardian", "acl-fix", "metadata"):
        stdlib = pathlib.Path(sysconfig.get_path("stdlib"))
        target = root / str(stdlib).lstrip("/")
        shutil.copytree(stdlib, target, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns("site-packages", "__pycache__",
                                                     "test", "tests", "idlelib", "tkinter"))
        for extension in (stdlib / "lib-dynload").glob("*.so"):
            libraries(extension)
    (root / "etc/passwd").write_text("root:x:0:0:root:/root:/bin/bash\n")
    (root / "etc/group").write_text("root:x:0:\n")
    (root / "etc/resolv.conf").write_text("")
    (root / "etc/nsswitch.conf").write_text("hosts: files dns\n")
    (root / "etc/mtab").symlink_to("/proc/mounts")
    for dest, source in CASES[args.case].items():
        text = (HERE / source).read_text()
        # Give the serial console time to emit the final verdict before reboot.
        if dest == "init":
            text = text.replace("echo b >", "sleep .1; echo b >")
        (root / dest).write_text(text)
        (root / dest).chmod(0o755)

    modules = pathlib.Path("/lib/modules") / args.kernel_version
    dependencies = {}
    for line in (modules / "modules.dep").read_text().splitlines():
        name, deps = line.split(":", 1)
        dependencies[name] = deps.split()
    for metadata in modules.glob("modules.*"):
        if metadata.is_file():
            copy(metadata)
    visited = set()

    def module(path):
        if path in visited:
            return
        visited.add(path)
        copy(modules / path)
        for dep in dependencies.get(path, []):
            module(dep)

    needed = ["fuse", "loop", "dm_mod"]
    if args.case in ("storage", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure"):
        needed += ["dm_snapshot"]
    if args.case in ("chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-publication-failure"):
        needed += ["dm_verity", "dm_zero"]
    if args.case in ("network", "network-policy"):
        needed += ["veth", "nf_tables", "nft_chain_nat", "nft_masq", "nft_ct"]
    for name in needed:
        path = subprocess.check_output(
            ["modinfo", "-k", args.kernel_version, "-n", name], text=True).strip()
        if path != "(builtin)":
            module(str(pathlib.Path(path).relative_to(modules)))
    shutil.copy2(pathlib.Path("/boot") / f"vmlinuz-{args.kernel_version}",
                 out / "vmlinuz")
    inventory = {"case": args.case, "kernel_version": args.kernel_version,
                 "files": {}, "symlinks": {}}
    for path in sorted(root.rglob("*")) + [out / "vmlinuz"]:
        relative = str(path.relative_to(out))
        if path.is_symlink():
            inventory["symlinks"][relative] = str(path.readlink())
        elif path.is_file():
            with path.open("rb") as stream:
                inventory["files"][relative] = hashlib.file_digest(stream, "sha256").hexdigest()
    if capture_sources(args.case) != sources:
        raise RuntimeError("Local fixture/build sources changed during packaging; discard this run")
    inventory["sources"] = {name: hashlib.sha256(data).hexdigest()
                            for name, data in sources.items()}
    for name, data in sources.items():
        target = out / "sources" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (out / "inputs.json").write_text(json.dumps(inventory, indent=2) + "\n")
    paths = [str(p.relative_to(root)) for p in sorted(root.rglob("*"))]
    archive = subprocess.run(["cpio", "--null", "-o", "--format=newc", "--owner=0:0"],
                             input=b"\0".join(p.encode() for p in paths) + b"\0",
                             cwd=root, capture_output=True, check=True)
    with (out / "initramfs.gz").open("wb") as stream:
        subprocess.run(["gzip", "-1", "-n"], input=archive.stdout, stdout=stream, check=True)
    (out / "cpio.log").write_bytes(archive.stderr)
    command = ["qemu-system-x86_64", "-enable-kvm", "-cpu", "host", "-m", "4096",
               "-smp", "2", "-nodefaults", "-no-reboot", "-display", "none",
               "-serial", "stdio", "-monitor", "none", "-kernel", str(out / "vmlinuz"),
               "-initrd", str(out / "initramfs.gz"), "-append",
               "console=ttyS0 rdinit=/init selinux=0 panic=-1 quiet"]
    (out / "command.json").write_text(json.dumps(command, indent=2) + "\n")
    if args.run:
        with (out / "results.txt").open("wb") as stream:
            subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT,
                           timeout=150, check=True)
        result = (out / "results.txt").read_text()
        validate_guest_exit(result)
        missing = [marker for marker in MARKERS[args.case]
                   if marker not in result]
        if missing:
            raise RuntimeError(f"Missing probe outcomes: {missing}; inspect {out}/results.txt")
        if args.case == "chain-long":
            for marker in ("CHAIN_RESTRICTED_COMMAND_PASS", "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE",
                           "FUSE_DERIVED_LAYER_SEALED"):
                if result.count(marker) != 3:
                    raise RuntimeError(f"Expected three rounds of {marker}")
            if "cannot redirect standard input" in result:
                raise RuntimeError("Background writer stdin setup failed")
        if args.case in ("chain-command-failure", "chain-publication-failure"):
            if any(marker in result for marker in ("FUSE_DERIVED_LAYER_SEALED", "DERIVED_CHAIN_RESTAGED", "CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS")):
                raise RuntimeError("Failed command reached layer sealing")
        if args.case == "acl":
            for marker in ("ACL_ALLOW_FAIL", "ACL_AFTER_CHMOD_ALLOW_PASS",
                           "ACL_INHERITANCE_FAIL"):
                if result.count(marker) != 2:
                    raise RuntimeError(f"Expected {marker} in both namespaces")
        if args.case == "network-policy":
            for marker in ("LOOPBACK_STUB_UNREACHABLE_CONFIRMED", "DNS_SEARCH_DOMAIN_PASS",
                           "NETWORK_RESTRICTED_CLIENT_PASS", "DNS_TCP_PASS",
                           "NAT_PRIVATE_DESTINATION_AND_DNS_UDP_PASS",
                           "IPV6_NAT_AND_AAAA_DNS_PASS",
                           "IPV6_BUILDER_ENDPOINT_BLOCKED_PASS"):
                if result.count(marker) != 2:
                    raise RuntimeError(f"Expected {marker} for both resolver layouts")
        if args.case == "acl-fix":
            if result.count("ACL_NONROOT_FSETID_SGID_PRESERVED") != 2:
                raise RuntimeError("Expected CAP_FSETID preservation in both kernel reference runs")
            if result.count("ACL_NONROOT_FSETID_SGID_CLEARED_LIMITATION") != 4:
                raise RuntimeError("Expected explicit capability limitation in all four FUSE runs")
            if result.count("ACL_ROOT_WITHOUT_FSETID_SGID_CLEARED\n") != 2:
                # Serial output normally uses CRLF, normalized by read_text().
                raise RuntimeError("Expected dropped-CAP_FSETID behavior in both kernel runs")
            if result.count("ACL_ROOT_WITHOUT_FSETID_SGID_PRESERVED_LIMITATION") != 4:
                raise RuntimeError("Expected root-without-capability limitation in all FUSE runs")
        if args.case == "metadata":
            if result.count("INHERITED_FLAGS_KERNEL_REMOUNT_PASS") != 6:
                raise RuntimeError("Inherited flags did not persist in all six cases")
            if result.count("METADATA_KERNEL_REMOUNT_MATCH_PASS") != 6:
                raise RuntimeError("Expected exact metadata persistence in all six runs")
            observations = [
                json.loads(line.removeprefix("METADATA_OBSERVATIONS "))
                for line in result.splitlines()
                if line.startswith("METADATA_OBSERVATIONS ")
            ]
            if len(observations) != 6:
                raise RuntimeError("Expected six complete metadata observations")
            for index, observed in enumerate(observations):
                capability = "0100000200040000000000000000000000000000"
                for operation in ("persist", "write", "chown", "chmod"):
                    if observed[f"cap-{operation}-set"] != {"ok": None}:
                        raise RuntimeError(f"Capability setup failed: {index} {operation}")
                    if observed[f"cap-{operation}-before"] != {"ok": capability}:
                        raise RuntimeError(f"Capability wire representation changed: {index} {operation}")
                    expected_cap = {"errno": "ENODATA"} if operation in ("write", "chown") else {"ok": capability}
                    if observed[f"cap-{operation}-after"] != expected_cap:
                        raise RuntimeError(f"Capability mutation semantics changed: {index} {operation}")
                for operation, expected_result in (("write", 12), ("chown", None), ("chmod", None)):
                    if observed[f"cap-{operation}-operation"] != {"ok": expected_result}:
                        raise RuntimeError(f"Capability file operation failed: {index} {operation}")
                for name in ("time-normal", "time-before-epoch", "time-after-2038"):
                    if observed[name] != {"ok": None, "exact": True}:
                        raise RuntimeError(f"Timestamp mismatch: {index} {name}")
                for name in ("user.fixture", "security.selinux", "nodump-set"):
                    if observed[name] != {"ok": None}:
                        raise RuntimeError(f"Metadata mismatch: {index} {name}")
                expected = {"errno": "EPERM"} if index % 3 == 2 else {"ok": None}
                for name in ("trusted.fixture", "device-node", "immutable-set", "append-set", "inherited-immutable-clear", "inherited-append-clear"):
                    if observed[name] != expected:
                        raise RuntimeError(f"Changed metadata boundary: {index} {name}")
                if index % 3 == 1 and observed != observations[index - 1]:
                    raise RuntimeError("Initial-userns FUSE differs from kernel operations")
        print(f"Recorded expected probe outcomes in {out}/results.txt.")
        print("ACL/fault cases deliberately reproduce defects; this is not a security verdict.")


if __name__ == "__main__":
    main()
