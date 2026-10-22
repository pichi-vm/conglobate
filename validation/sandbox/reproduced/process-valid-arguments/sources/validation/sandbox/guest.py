#!/usr/bin/env python3
"""Package and run disposable sandbox probes. No host mounts or root required."""
import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sysconfig

HERE = pathlib.Path(__file__).resolve().parent
CASES = {
    "chain-worker-oom": {"init": "chain-worker-oom/init", "chain-worker": "chain-worker-oom/worker",
                         "controller.py": "chain-worker-oom/controller.py", "cancel-driver.py": "chain-worker-oom/driver.py",
                         "command-launch": "chain-active-cancel/launch", "command": "chain-active-cancel/command",
                         "verify": "chain/verify"},
    **{f"chain-{kind}-limit": {"init": f"chain-{kind}-limit/init", "controller.py": f"chain-{kind}-limit/controller.py",
                              "chain-worker": "chain-command/worker", "command-launch": "chain-active-cancel/launch",
                              "verify": "chain/verify"} for kind in ("fd", "memory")},
    "chain-pids-limit": {"init": "chain-pids-limit/init", "controller.py": "chain-pids-limit/controller.py",
                         "chain-worker": "chain-command/worker", "command-launch": "chain-active-cancel/launch",
                         "verify": "chain/verify"},
    "cyclic-fuse": {p: f"cyclic-fuse/{p}" for p in ("init", "worker", "client.py")},
    "copy-tests": {"init": "copy-tests/init"},
    "chain-resolver": {"init": "chain-resolver/init", "chain-worker": "chain-resolver/worker",
                       "command-launch": "chain-resolver/launch", "command": "chain-resolver/command",
                       "verify": "chain-resolver/verify"},
    "resolver-lifecycle": {p: f"resolver-lifecycle/{p}" for p in ("init", "launch", "driver.py")},
    "split-dns": {"init": "split-dns/init", "client.py": "split-dns/client.py",
                  "split-server.py": "split-dns/server.py", "launch": "network-policy/launch",
                  "server.py": "network-policy/server.py", "stub.py": "network-policy/stub.py",
                  "listener.py": "network-policy/listener.py"},
    "carapace-activation-recovery": {"init": "carapace-activation/recovery-init"},
    "carapace-activation-death": {"init": "carapace-activation/death-init"},
    "carapace-activation": {"init": "carapace-activation/init"},
    "activation-recovery": {"init": "activation-recovery/init", "probe.py": "activation-recovery/probe.py"},
    "chain-supervisor-death": {"init": "chain-supervisor-death/init", "chain-worker": "chain-active-cancel/worker",
                               "command-launch": "chain-active-cancel/launch", "command": "chain-active-cancel/command",
                               "cancel-driver.py": "chain-supervisor-death/driver.py", "guardian.py": "chain-supervisor-death/guardian.py",
                               "verify": "chain/verify"},
    "chain-blocked-cancel": {"init": "chain-blocked-cancel/init", "chain-worker": "chain-blocked-cancel/worker",
                             "command-launch": "chain-active-cancel/launch", "command": "chain-active-cancel/command",
                             "cancel-driver.py": "chain-blocked-cancel/driver.py", "verify": "chain/verify"},
    "pressure": {"init": "pressure/init", "worker": "pressure/worker"},
    "acl-protocol-unavailable": {"init": "acl-protocol/unavailable-init"},
    "acl-protocol": {"init": "acl-fix/init", "worker": "acl-fix/worker", "probe.py": "acl-fix/probe.py"},
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
    "policy": {p: f"policy/{p}" for p in ("init", "launch", "probe.py", "driver.py", "process-probe.py", "cross-process.py")},
    "copy": {p: f"copy/{p}" for p in ("init", "worker", "launch", "command")},
    "export": {p: f"copy/{p}" for p in ("init", "worker", "launch", "command")},
    "chain-active-cancel": {"init": "chain-active-cancel/init", "chain-worker": "chain-active-cancel/worker",
                            "command-launch": "chain-active-cancel/launch", "command": "chain-active-cancel/command",
                            "cancel-driver.py": "chain-active-cancel/driver.py", "verify": "chain/verify"},
    "chain-publication-cancel": {"init": "chain-publication/cancel-init", "chain-worker": "chain-command/worker",
                                 "command-launch": "chain-command/launch", "command": "chain-command/command", "verify": "chain/verify"},
    "chain-acl": {"init": "chain-acl/init", "chain-worker": "chain-command/worker",
                  "command-launch": "chain-command/launch", "command": "chain-acl/command", "verify": "chain-acl/verify"},
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
    "chain-worker-oom": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "WORKER_OOM_TWO_WRITERS_LIVE",
                         "WORKER_OOM_REQUEST_QUEUED", "WORKER_OOM_KERNEL_KILL_EVENT_CONFIRMED",
                         "WORKER_OOM_DAEMON_REAPED_137_UNMOUNTED", "worker failed: exit status: 125",
                         "WORKER_OOM_SUPERVISOR_SURVIVED_CGROUP_CLEANUP_PASS", "WORKER_OOM_FUSE_CONNECTIONS_RELEASED",
                         "WORKER_OOM_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "chain-fd-limit": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_COMMAND_FD_EMFILE_CONFIRMED",
                       "worker failed: exit status: 125", "CHAIN_FD_LIMIT_EVENT_AND_CGROUP_CLEANUP_PASS",
                       "FD_LIMIT_FUSE_CONNECTIONS_RELEASED", "FD_LIMIT_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "chain-memory-limit": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_COMMAND_MEMORY_PRESSURE_STARTED",
                           "worker failed: exit status: 137", "CHAIN_MEMORY_LIMIT_EVENT_AND_CGROUP_CLEANUP_PASS",
                           "MEMORY_LIMIT_FUSE_CONNECTIONS_RELEASED", "MEMORY_LIMIT_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "chain-pids-limit": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_COMMAND_PIDS_EAGAIN_CONFIRMED",
                         "worker failed: exit status: 125", "CHAIN_PIDS_LIMIT_EVENT_AND_CGROUP_CLEANUP_PASS",
                         "PIDS_LIMIT_FUSE_CONNECTIONS_RELEASED", "PIDS_LIMIT_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "cyclic-fuse": ["CYCLIC_CALLBACK_DEPENDENCY_QUEUED", "CYCLIC_CLIENT_ABORT_ERROR",
                    "CYCLIC_WORKER_AND_CLIENT_REAPED", "CYCLIC_ABORT_RELEASES_CALLBACK_AND_CLIENT_PASS"],
    "copy-tests": ["test result: ok. 5 passed; 0 failed; 0 ignored", "COPY_ROOT_TESTS_PASS"],
    "chain-resolver": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_RESOLVER_READONLY_INJECTION_PASS",
                       "CHAIN_RESOLVER_WORKER_VIEW_RESTORED_PASS", "CHAIN_RESOLVER_SEALED_IMAGE_ORIGINAL_PASS",
                       "CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS", "CHAIN_RESOLVER_PUBLICATION_PASS"],
    "resolver-lifecycle": ["RESOLVER_SETUP_CANCELLATION_PASS before-bind",
        "RESOLVER_SETUP_CANCELLATION_PASS after-bind", "RESOLVER_SETUP_CANCELLATION_PASS after-readonly",
        "RESOLVER_SETUP_FILES_AND_PARENT_MOUNTS_PRESERVED"],
    "split-dns": ["SPLIT_FLAT_LIST_LIMIT_CONFIRMED 172.27.0.2",
                  "SPLIT_FLAT_LIST_LIMIT_CONFIRMED 172.27.0.3", "SPLIT_ROUTED_UDP_TCP_PASS",
                  "SPLIT_ROUTE mirror.private.test 172.27.0.3 tcp", "SPLIT_DNS_MATRIX_PASS"],
    "carapace-activation-recovery": ["CREATED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS",
        "LOADED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS", "RESUMED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS",
        "OPENED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS", "test result: ok. 4 passed; 0 failed; 0 ignored",
        "RECOVERY_HELD_INACTIVE_PRESERVED", "RECOVERY_MISMATCHED_ACTIVE_PRESERVED",
        "RECOVERY_DEATH_AFTER_REMOVE_RESTART_PASS",
        "CARAPACE_RECOVERY_CANDIDATE_AND_CLEANUP_PASS"],
    "carapace-activation-death": ["CREATED_DEATH_STALE_MAPPING_BLOCKS_REUSE",
                                  "LOADED_DEATH_STALE_MAPPING_BLOCKS_REUSE",
                                  "OPENED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS",
                                  "test result: ok. 4 passed; 0 failed; 0 ignored",
                                  "RESUMED_DEATH_REUSE_AND_DEFERRED_CLEANUP_PASS",
                                  "test result: ok. 1 passed; 0 failed; 0 ignored",
                                  "CARAPACE_DEATH_PROBE_AND_CLEANUP_PASS"],
    "carapace-activation": ["test result: ok. 4 passed; 0 failed; 0 ignored",
                            "CARAPACE_ACTIVATION_TESTS_AND_CLEANUP_PASS"],
    "activation-recovery": ["ACTIVATION_RECOVERY_PASS created", "ACTIVATION_RECOVERY_PASS active",
                            "ACTIVATION_RECOVERY_OTHER_LEASES_PRESERVED", "ACTIVATION_RECOVERY_RELEASED_LEASE_PASS",
                            "ACTIVATION_RECOVERY_DEFERRED_READER_PASS", "ACTIVATION_RECOVERY_MATRIX_PASS"],
    "chain-supervisor-death": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "SUPERVISOR_DEATH_TWO_WRITERS_LIVE",
                               "SUPERVISOR_DEATH_OBSERVED", "SUPERVISOR_DEATH_SHARED_CONSUMERS_PINNED",
                               "SUPERVISOR_DEATH_SHARED_CONSUMERS_SURVIVE", "SUPERVISOR_DEATH_PROCESSES_AND_FUSE_RELEASED",
                               "SUPERVISOR_DEATH_OWNED_DEVICE_RECOVERY_PASS", "SUPERVISOR_DEATH_DEFERRED_DEVICE_RELEASE_PASS", "SUPERVISOR_DEATH_FUSE_CONNECTIONS_RELEASED",
                               "SUPERVISOR_DEATH_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "chain-blocked-cancel": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "BLOCKED_CANCEL_TWO_WRITERS_LIVE",
                             "BLOCKED_CANCEL_REQUEST_QUEUED", "BLOCKED_CANCEL_COMMAND_REAPED",
                             "BLOCKED_CANCEL_WORKER_REAPED_UNMOUNTED", "worker failed: exit status: 125",
                             "BLOCKED_CANCEL_ABORT_BEFORE_REAP_PASS", "BLOCKED_CANCEL_FUSE_CONNECTIONS_RELEASED",
                             "BLOCKED_CANCEL_NO_SEAL_DEVICE_CLEANUP_PASS", "/work/base: OK"],
    "chain-active-cancel": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "ACTIVE_CANCEL_TWO_WRITERS_LIVE",
                            "ACTIVE_CANCEL_WRITERS_STOPPED", "worker failed: exit status: 125",
                            "ACTIVE_CANCEL_NO_SEAL_DEVICE_CLEANUP_PASS", "ACTIVE_CANCEL_FUSE_CONNECTIONS_RELEASED",
                            "/work/base: OK"],
    "pressure": ["PRESSURE_QUEUED abort 32", "PRESSURE_QUEUED clients-first 32",
                 "PRESSURE_CLIENTS_REAPED abort", "PRESSURE_CLIENTS_REAPED clients-first",
                 "PRESSURE_WORKER_REAPED_UNMOUNTED abort", "PRESSURE_WORKER_REAPED_UNMOUNTED clients-first",
                 "PRESSURE_CLEANUP_PASS abort", "PRESSURE_CLEANUP_PASS clients-first",
                 "PRESSURE_ABORT_ERRORS_VERIFIED", "PRESSURE_NEW_REQUEST_REJECTED",
                 "PRESSURE_MATRIX_PASS"],
    "acl-protocol-unavailable": ["UNSUPPORTED_EXTENSION_REJECTED_CLEANUP_PASS",
                                 "UNSUPPORTED_EXTENSION_WORKER_STATUS 32",
                                 "fuse: unknown connection 'want' flags: 0x08000000"],
    "acl-protocol": ["ACL_COMMON_SEMANTICS_MATRIX_PASS",
                     "ACL_REFERENCE_MATCH_PASS initial metadata-csum",
                     "ACL_REFERENCE_MATCH_PASS mapped metadata-csum",
                     "ACL_REFERENCE_MATCH_PASS initial without-metadata-csum",
                     "ACL_REFERENCE_MATCH_PASS mapped without-metadata-csum"],
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
    "policy": ["PROCESS_VALID_ARGUMENT_CONTROLS_PASS", "PROCESS_VALID_ARGUMENT_DENIALS_PASS",
               "CROSS_PROCESS_VALID_CONTROLS_PASS", "CROSS_PROCESS_MAPPED_DENIALS_PASS",
               "CROSS_PROCESS_PARENT_CANARIES_UNCHANGED_PASS",
               "POLICY_SUPERVISOR_MOUNTS_AND_RUNTIME_BACKING_UNCHANGED_PASS",
               "POLICY_GLOBAL_PROC_CONTROL_OPENS_DENIED_PASS",
               "POLICY_RUNTIME_MOUNTS_PRIVATE_AND_RESTRICTED_PASS", "NAMESPACE_ONLY_REAL_KEY_READ_PASS", "POLICY_REAL_KEY_READ_DENIED_PASS",
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
    "chain-publication-cancel": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_RESTRICTED_COMMAND_PASS",
                                 "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE", "FUSE_DERIVED_LAYER_SEALED",
                                 "CHAIN_PRIVATE_INVENTORY_FINALIZED_BEFORE_CANCEL",
                                 "CHAIN_CANCELLED_PUBLICATION_NO_OUTPUT_OR_TEMP_PASS",
                                 "CHAIN_CANCELLED_PUBLICATION_STORAGE_CLEANUP_PASS"],
    "chain-acl": ["COMMITTED_LOCKED_CHAIN_INPUTS_PASS", "CHAIN_RESTRICTED_COMMAND_PASS",
                  "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE", "CHAIN_RESTRICTED_ACL_MASK_PASS",
                  "CHAIN_ACL_KERNEL_REMOUNT_PASS", "CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS",
                  "BASE_SCUTE_UNCHANGED_PASS", "CHAIN_PUBLICATION_MATRIX_PASS"],
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
    if case in ("chain-pids-limit", "chain-fd-limit", "chain-memory-limit"):
        paths.add(HERE / case / "command.c")
    if case == "cyclic-fuse":
        paths.add(HERE / "cyclic-fuse/worker.c")
    if case == "copy-tests":
        paths.add(repository / "tests/copy.rs")
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
    parser.add_argument("--protocol-worker-build", type=pathlib.Path,
                        help="experimental paired ACL worker/library override for chain-acl or metadata")
    parser.add_argument("--activation-tests-build", type=pathlib.Path)
    parser.add_argument("--fuse-headers", type=pathlib.Path, help="libfuse headers for cyclic-fuse fixture compilation")
    args = parser.parse_args()
    if args.case == "cyclic-fuse" and args.fuse_headers is None:
        parser.error("cyclic-fuse requires --fuse-headers")
    if args.case in ("carapace-activation", "carapace-activation-death", "carapace-activation-recovery") and args.activation_tests_build is None:
        parser.error("carapace-activation requires --activation-tests-build")
    if args.case == "chain-acl" and args.protocol_worker_build is None:
        parser.error("chain-acl requires --protocol-worker-build")
    if args.protocol_worker_build is not None and args.case not in ("chain-acl", "metadata"):
        parser.error("--protocol-worker-build is scoped to chain-acl or metadata")
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
        library_env = os.environ | {"LD_LIBRARY_PATH": str((args.protocol_worker_build or args.worker_build).resolve())} if args.case in ("acl-protocol", "acl-protocol-unavailable", "chain-acl") or args.protocol_worker_build is not None else None
        result = subprocess.run(["ldd", str(path)], capture_output=True, text=True, env=library_env)
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
    if args.case in ("chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
        tools += ["git"]
    if args.case in ("network", "network-policy", "split-dns"):
        tools += ["ip", "nft", "python3", "sysctl"]
    elif args.case in ("cyclic-fuse", "resolver-lifecycle", "activation-recovery", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "keys", "policy", "loader", "lifecycle", "guardian", "acl-fix", "metadata", "acl-protocol"):
        tools += ["python3"]
    for tool in tools:
        path = shutil.which(tool)
        if path is None:
            raise RuntimeError(f"Required host executable missing: {tool}")
        copy(path)
        libraries(path)
    if args.case == "copy-tests":
        command = ["cargo", "test", "--locked", "--manifest-path", str(HERE.parents[1] / "Cargo.toml"),
                   "--test", "copy", "--no-run", "--message-format=json"]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        (out / "test-build.log").write_text(result.stderr)
        result.check_returncode()
        records = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
        binary, = [pathlib.Path(record["executable"]) for record in records
                   if record.get("reason") == "compiler-artifact" and record.get("executable")
                   and record["target"]["name"] == "copy" and record["profile"]["test"]]
        shutil.copy2(binary, root / "copy-tests")
        libraries(binary)
        (out / "test-build-command.json").write_text(json.dumps(command, indent=2) + "\n")
    if args.case == "cyclic-fuse":
        headers = args.fuse_headers.resolve()
        shutil.copytree(headers, out / "fuse-headers")
        binary = root / "cyclic-worker"
        command = ["cc", "-Wall", "-Wextra", "-Werror", "-D_FILE_OFFSET_BITS=64",
                   "-I" + str(headers), str(HERE / "cyclic-fuse/worker.c"),
                   "/lib64/libfuse3.so.4", "-o", str(binary)]
        subprocess.run(command, check=True)
        libraries(binary)
        (out / "worker-build.json").write_text(json.dumps({"command":command,
            "headers_sha256":{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(headers.glob("*.h"))}}, indent=2) + "\n")
    if args.case in ("chain-pids-limit", "chain-fd-limit", "chain-memory-limit"):
        binary = root / "command"
        subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", str(HERE / args.case / "command.c"),
                        "-o", str(binary)], check=True)
        libraries(binary)
    if args.case == "loader":
        binary = root / "loader-canary.so"
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                        str(HERE / "loader/canary.c"), "-o", str(binary)], check=True)
        libraries(binary)
    if args.case in ("finalize", "publication", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
        if args.worker_build is None:
            raise RuntimeError("finalize requires --worker-build from finalize/build.py")
        for name in ("baseline", "baseline-close", "patched", "patched-close"):
            binary = args.worker_build / name
            shutil.copy2(binary, root / name)
            libraries(binary)
    if args.case in ("carapace-activation", "carapace-activation-death", "carapace-activation-recovery"):
        binary = args.activation_tests_build / "activation-tests"
        catalog = json.loads((args.activation_tests_build / "inputs.json").read_text())
        if hashlib.sha256(binary.read_bytes()).hexdigest() != catalog["binary_sha256"]:
            raise RuntimeError("activation test binary differs from build catalog")
        shutil.copy2(binary, root / "activation-tests")
        shutil.copy2(args.activation_tests_build / "inputs.json", out / "dependency-build-inputs.json")
        libraries(binary)
    if args.case == "activation-recovery":
        helper = HERE / "publication/helper"
        target = HERE / "copy/helper/target"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(helper / "Cargo.toml"), "--target-dir", str(target), "--bin", "recover"], check=True)
        binary = target / "debug/recover"
        shutil.copy2(binary, root / "recovery-helper")
        libraries(binary)
    if args.case == "chain-acl":
        binary = args.protocol_worker_build / "acl-patched"
        shutil.copy2(binary, root / "patched")
        libraries(binary)
        shutil.copy2(args.protocol_worker_build / "libfuse3.so.4", root / "usr/lib64/libfuse3.so.4")
    if args.case in ("publication", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
        helper = HERE / "publication/helper"
        target = HERE / "copy/helper/target"
        subprocess.run(["cargo", "build", "--locked", "--manifest-path",
                        str(helper / "Cargo.toml"), "--target-dir", str(target)], check=True)
        binary = target / ("debug/chain" if args.case in ("chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl") else "debug/conglobate-publication-probe")
        shutil.copy2(binary, root / "publication-helper")
        libraries(binary)
    if args.case in ("acl-fix", "metadata", "acl-protocol", "acl-protocol-unavailable"):
        if args.worker_build is None:
            raise RuntimeError("acl-fix requires --worker-build from acl-fix/build.py")
        binary = (args.protocol_worker_build or args.worker_build) / "acl-patched"
        shutil.copy2(binary, root / "acl-patched")
        libraries(binary)
    if args.case in ("acl-protocol", "acl-protocol-unavailable") or (args.case == "metadata" and args.protocol_worker_build is not None):
        shutil.copy2((args.protocol_worker_build or args.worker_build) / "libfuse3.so.4", root / "usr/lib64/libfuse3.so.4")
    if args.case in ("entry", "loader", "policy", "lifecycle", "guardian", "copy", "export", "network-policy", "split-dns", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
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
    if args.case in ("cyclic-fuse", "resolver-lifecycle", "activation-recovery", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "network", "network-policy", "split-dns", "keys", "policy", "loader", "lifecycle", "guardian", "acl-fix", "metadata", "acl-protocol"):
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
    if args.case in ("activation-recovery", "carapace-activation", "carapace-activation-death", "carapace-activation-recovery"):
        needed += ["dm_zero"]
    if args.case in ("storage", "chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
        needed += ["dm_snapshot"]
    if args.case in ("chain", "chain-command", "chain-command-failure", "chain-long", "chain-publication", "chain-resolver", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death", "chain-publication-cancel", "chain-acl"):
        needed += ["dm_verity", "dm_zero"]
    if args.case in ("network", "network-policy", "split-dns"):
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
        if args.case == "chain-publication-cancel" and "CHAIN_COMMITTED_INVENTORY_PUBLICATION_PASS" in result:
            raise RuntimeError("Cancelled publication reached success")
        if args.case == "chain-long":
            for marker in ("CHAIN_RESTRICTED_COMMAND_PASS", "CHAIN_COMMAND_DESCENDANTS_STOPPED_WORKER_ALIVE",
                           "FUSE_DERIVED_LAYER_SEALED"):
                if result.count(marker) != 3:
                    raise RuntimeError(f"Expected three rounds of {marker}")
            if "cannot redirect standard input" in result:
                raise RuntimeError("Background writer stdin setup failed")
        if args.case in ("chain-command-failure", "chain-publication-failure", "chain-active-cancel", "chain-pids-limit", "chain-fd-limit", "chain-memory-limit", "chain-blocked-cancel", "chain-worker-oom", "chain-supervisor-death"):
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
        if args.case == "acl-protocol":
            if "_LIMITATION" in result:
                raise RuntimeError("Protocol-aware ACL worker still differs from kernel")
            for marker in ("ACL_NONROOT_FSETID_SGID_PRESERVED", "ACL_ROOT_WITHOUT_FSETID_SGID_CLEARED"):
                if result.count(marker) != 6:
                    raise RuntimeError(f"Expected {marker} in all six cases")
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
                # Extent limitations are explicit observations, never counted as
                # equivalent kernel semantics. Existing metadata stays strict.
                kernel = index % 3 == 0
                expected_io = {"io-reflink": {"errno": "ENOTSUP"}}
                for operation in ("allocate", "punch", "zero", "collapse"):
                    expected_io[f"io-{operation}"] = (
                        {"errno": "ENOTSUP"} if operation == "collapse" and not kernel
                        else {"ok": None})
                    data = 32768 if operation == "punch" and kernel else 16384
                    hole = {"allocate": 131072, "zero": 65536,
                            "punch": 16384 if kernel else 65536,
                            "collapse": 49152 if kernel else 65536}[operation]
                    expected_io[f"io-{operation}-blocks-delta"] = {
                        "allocate": 128, "punch": -32, "zero": 0,
                        "collapse": -32 if kernel else 0}[operation]
                    expected_io[f"io-{operation}-seek-data"] = {"ok": data}
                    expected_io[f"io-{operation}-seek-hole"] = {"ok": hole}
                actual_io = {key: value for key, value in observed.items() if key.startswith("io-")}
                if actual_io != expected_io:
                    raise RuntimeError(f"Changed extent semantics: {index} {actual_io}")
                metadata = lambda row: {key: value for key, value in row.items() if not key.startswith("io-")}
                if index % 3 == 1 and metadata(observed) != metadata(observations[index - 1]):
                    raise RuntimeError("Initial-userns FUSE differs from kernel metadata operations")
            if result.count("UNALIGNED_RANGE_CONTENT_CHECKS_PASS") != 6:
                raise RuntimeError("Expected unaligned content checks in every configuration")
            if result.count("BOUNDED_EXTENT_CONTENT_CHECKS_PASS") != 6:
                raise RuntimeError("Expected extent content checks in every configuration")
        print(f"Recorded expected probe outcomes in {out}/results.txt.")
        print("ACL/fault cases deliberately reproduce defects; this is not a security verdict.")


if __name__ == "__main__":
    main()
