Two complete builds with identical file-backed base content
=========================================================
Run from repository root:
  python3 validation/sandbox/guest.py chain-concurrent \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/conglobate-finalize-workers \
    --out /tmp/NEW-concurrent-run --run

Disposable guest only; no host disk/share/network. Builds have separate private
mount namespaces and workspace directories, with the existing actual Chain,
Snapshot, restricted command, kernel remount verifier and Record publication.
A single formatted/populated base is copied byte-for-byte into both workspaces.
Both FUSE workers must reach a bounded trusted launch gate before either command
runs. DM tables must show two distinct writable snapshot origins, two base
snapshots sharing one zero origin, and exactly one CARAPACE-z mapping.

This distinction matters: carapace assemble.rs binds verity/snapshot names to
content AND backing device IDs. Separately staged identical regular files get
separate loop attachments/mapping stacks. Identical content alone does not
coalesce the entire stack. This test exercises the shared zero mapping and
concurrent production scratch/snapshot ownership, not shared block inputs.

Release A while B retains its mounted worker and snapshot. Require A's verified
inventory publication and helper exit while B remains active without output.
Then release B and require its full chain verification/publication. Both base
copies must remain byte-identical. Require no attached loop or DM mappings after
both complete. The harness requires two occurrences of every principal build
success marker, not merely one global success string. Each launch gate waits
at most 60 seconds; the whole guest has the existing 150-second timeout.

Both trusted workers currently use the fixture's same mapped UID range. This
is storage concurrency evidence, NOT cross-build security/UID allocation proof.
No simultaneous failure, COW recovery, shared full block-backed chain, final
executor integration or production security qualification is established here.

Initial fixture failures are retained separately: v1 lacked child log capture;
v2 exposed workspace permissions unlike the original tmpfs root; v3 reached
overlap but lacked awk. The final fixture uses shell table parsing and supplies
cmp explicitly. These are harness defects, not reported storage defects.
