Restricted commands inside an authenticated carapace snapshot
===========================================================

Build the experimental workers as in ../finalize/README.txt. Run:
  python3 validation/sandbox/guest.py chain-command \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-COMMAND --run
  python3 validation/sandbox/guest.py chain-command-failure \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-FAILURE --run

This extends ../chain/README.txt using its real Scute/Chain/Snapshot helper.
The 64MiB ext4 base contains a small shell/tool runtime and the command fixture
before import, so those bytes contribute to the pinned base root. The trusted
worker mounts the snapshot and launches the command through entry-helper with
the current restrictions, private mount/PID/network/IPC/UTS namespaces, private
proc/dev, pivot_root and old-root detachment. The owning user namespace maps
target IDs to 100000..165535. Stdin is /dev/null; output is piped to the worker.

The command checks PID1, no_new_privs/seccomp, absence of context, helper,
backing-device and old-root paths, then reads inherited content and changes the
filesystem. It creates a finite background writer before exiting. The trusted
worker checks a nonempty heartbeat stops changing and the FUSE daemon remains
alive, then unmounts and reaps that daemon. Pipeline failure propagation is
enabled in the worker shell.

Successful path:
- Restricted command writes and target UID/GID/mode operations pass.
- Background writing stops before Snapshot::finish.
- The derived layer seals, unordered layers reconstruct, and kernel ext4
  verifies the resulting files and ownership.
- Original raw/scute digests remain unchanged.
- Post-stage corruption still produces EIO; restoring bytes permits reading.

Failed-command path:
- The measured fixture command has an added explicit exit7.
- The worker propagates that exact status through teardown.
- The diagnostic Rust helper deliberately asserts worker success; it therefore
  exits101 with "worker failed: exit status: 7" before Snapshot::finish.
- The runner requires that diagnostic and rejects any sealing/restaging marker.
- The guest asserts no materialized result and, after bounded cleanup, no dm
  mappings or attached loop devices remain.

The expected assertion is a test boundary, not a proposed production error API.
No host shares, disks or NICs are exposed. The command is bounded test code,
not an exploit or a representative distribution build.

Evidence:
  ../reproduced/chain-command/{inputs.json,command.json,results.txt,helper-sources.json}
  ../reproduced/chain-command-failure/ (same files)

Remaining scope:
This connects restricted execution, normal descendant cleanup, trusted FUSE
finalization and authenticated layer creation. It does not exercise recipe YAML
parsing, Git/refs.lock resolution, inventory publication in that same run,
supervisor cancellation/death during this path, or every backend fault. Those
gates remain distinct. Existing ACL/metadata compatibility blockers remain.
