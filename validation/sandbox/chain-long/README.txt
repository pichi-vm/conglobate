Four-layer authenticated chain with three restricted command rounds
=================================================================

Prerequisites: experimental workers from ../finalize/README.txt.
Run:
  python3 validation/sandbox/guest.py chain-long \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-LONG --run

This extends ../chain-command using the same trusted worker, restricted entry
helper and actual Chain/Snapshot APIs. The helper's --four-layers mode builds
one imported base plus three successive writable snapshot layers. Every round
reads the previous round counter and appends its number to an on-image history.
The counter and history are checked after reconstructing the full chain and
mounting its materialized filesystem through kernel ext4.

Each round checks that all previously sealed scute bytes remain unchanged.
The manifest lists layers in reverse order; restaging must recover the exact
root and parent sequence. Two bounded manifest variations preserve the pinned
top root but omit or duplicate a middle layer. They must fail specifically for
an incomplete authenticated chain or duplicate scute roots, respectively.
The unchanged baseline passes. Post-stage data corruption must still cause
EIO, and restoring that byte must permit a full filesystem-extent read.

The runner requires exactly three command, writer-quiescence and sealing
markers. Each command clears its heartbeat first, so a previous round cannot
satisfy the next round's nonempty-heartbeat check. Background stdin explicitly
uses the already-open descriptor; no /dev/null pathname is needed in the image.

Evidence is retained in ../reproduced/chain-long. The first exploratory run
passed but exposed Bash's implicit background /dev/null redirection warning.
The retained rerun uses explicit stdin and rejects that warning.

Scope: this is one four-layer ext4 chain, not a maximum-depth/performance test,
fuzzing campaign or complete structural-format qualification. It does not
combine YAML/Git/lock/inventory publication or cancellation with this path.
The known ACL and metadata backend limitations remain.

Shared-helper regression: ../reproduced/chain-command-failure-long-regression
retains a fresh failed-command run after these changes. Explicit exit7 still
stops before sealing; all dm mappings and loop attachments are cleaned up.
Rust helper clippy --all-targets -- -D warnings and shell/Python syntax checks
pass.
