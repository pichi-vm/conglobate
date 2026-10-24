# Rewrite preservation and recovery

The full pre-retirement tree and complete Git history are stored on persistent
local storage at:

`/storage/conglobate-validation/final-preservation/`

- `worktree.tar.zst`: all 84,489 working-tree entries, including uncommitted
  review streams, old implementations, nested validation build caches, reports,
  failures, source/context snapshots and user work. Only `.git` and the root
  `target` build cache are excluded. Every entry was compared to its original;
  regular-file content hashes and symlink targets matched.
- `reference.bundle`: complete committed history and refs at the preservation
  snapshot, verified by Git. `final-history.bundle` supplements it with the
  retirement commits; `final-ci/` holds final workflow and bundle receipts.
- `external.tar.zst`: original external validation references, all maintained
  validation run directories and the independent initial preservation archive.
  All 140,118 entries were verified against the originals; no baseline input
  disappeared.
- `worktree-manifest.json`, `external-roots.json`, `external-unavailable.json`,
  verification receipts and preservation scripts describe the exact scope.
  No path recorded as present at the initial baseline has disappeared. Entries
  already absent or patterns at that baseline remain explicitly recorded.

The worktree archive SHA-256 is
`dd4bebf04972a977fd55c50aa2f68c31aefda51d235204903b4ca97ff14be7f3`.
The external archive SHA-256 is
`f904e1b5d0292dacfe500782884bcbbf26fb795d04eea0b304ffa6a599916fda`.
Verification receipts are also retained in `evidence/archival/`.
These are local durable copies, not a claim of remote backup. Keep this directory
when reclaiming temporary validation data. Selected compact acceptance evidence
is retained in this repository under `evidence/`.

To inspect or recover, extract into new empty directories; do not extract the
external archive over the live filesystem:

```sh
mkdir /path/to/recovered-worktree /path/to/recovered-external
tar --zstd -xf /storage/conglobate-validation/final-preservation/worktree.tar.zst \
  -C /path/to/recovered-worktree
tar --zstd -xf /storage/conglobate-validation/final-preservation/external.tar.zst \
  -C /path/to/recovered-external
git clone /storage/conglobate-validation/final-preservation/reference.bundle \
  /path/to/recovered-history
```

External paths are archived relative to `/`; for example `/var/tmp/X` becomes
`var/tmp/X` under the recovery directory. Preserve the original snapshots when
adapting old scripts' absolute paths. Historical evidence must be interpreted
using its source version and trust model, not treated as current qualification.
