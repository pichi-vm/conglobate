# Hosted CI evidence for the linear workflow

Commit d5e77e652445f66dcba0597cbc9cd52285c35d78:
- Lifecycle run 40043887757 completed successfully.
- All six configured native-amd64 guest cases report passing checks and guest
  shutdown: cache-storage, contained, runtime-scope, runtime-private-namespace,
  runtime-changed-namespace, clone-filter.
- Each guest source archive's 146 selected production/fixture files matches the
  tested commit. See source-comparison.json. Small evidence is retained here;
  large kernel/initramfs assets are locally downloaded with hashes, not durably
  archived for M8.
- Rootless run 40043887779: Rust job 120721017603 completed successfully (format,
  strict Clippy, tests, doctests); dependency job was still installing checkers
  at the captured observation. No overall rootless-workflow success claimed yet.

These are passing configured cases, not full lifecycle, distro, architecture or
confidential-platform qualification. The independent full CLI composition result
is under linear-pipeline-validation. No new tests were started to obtain these
logs. Architecture's UID/GID-range question remains unanswered; no gate advances.

Final observation: rootless run 40043887779 completed successfully, including
both architecture dependency-policy checks and advisory checking. Raw final
workflow metadata/logs are rootless-final.json and rootless.log. Both workflows
are green at d5e77e6. The earlier live observation is retained as history. Later
local commits contain review/evidence changes only; do not call them separately
CI-qualified revisions.
