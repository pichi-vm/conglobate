Committed inputs through restricted execution to inventory publication
=====================================================================

Build workers as in ../finalize/README.txt, then run:
  python3 validation/sandbox/guest.py chain-publication \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-PUBLICATION --run
  python3 validation/sandbox/guest.py chain-publication-failure \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-WORKERS --out /tmp/NEW-CHAIN-FAILURE --run

This connects the existing authenticated chain/command fixture with actual
Inputs::resolve, inventory::Record, Artifact::finalize and Artifact::publish.
The helper creates an actual Git commit containing:
  pichi.build/carapace.yaml: from base; one offline run of /command
  pichi.build/refs.lock: the SHA256 of a generated OCI index
The index selects the imported base scute's manifest for pichi/amd64. It uses
the required index format with one platform entry, not an invented image pin.
The measured base contains /command and its interpreter/runtime.

The helper resolves the commit and locked cache through production input APIs,
checks the parsed recipe is exactly the supported single /command step, and
stages the resolved base. It then changes the caller's working-tree recipe to
an invalid reference. The captured committed recipe continues to determine
execution and inventory; no later source read is allowed to substitute it.

Execution uses the same candidate restricted launcher and mapped trusted FUSE
worker as chain-command. The command sees no /work, context, backing device or
trusted helper path. Descendant quiescence, worker unmount/reap and successful
finalization precede sealing. Kernel ext4 checks resulting files, ownership
and mode; input raw/scute hashes remain unchanged. The inherited and produced
scutes reconstruct and pass the existing corruption/restoration checks.

Afterward Record constructs the inventory from captured Inputs and actual
output Manifest, rather than synthetic commit/tree/recipe values. Assertions:
- committed recipe digest survives the working-tree change;
- input role is carapace-base and root matches the imported base;
- output scutes have inherited/produced origins and the final root;
- builder executable digest matches the running trusted probe;
- commit and tree identities match captured Git objects;
- published deterministic CBOR decodes and re-encodes identically;
- all published layer blobs verify and reconstruct the complete final chain.

The failure variant commits the same recipe but pins a base whose /command
ends with exit7. The trusted worker propagates failure; the diagnostic Rust
assertion stops before sealing/inventory/publication. The guest requires no
published directory or materialized result and no remaining dm/loop devices.
This expected test assertion is not a proposed production failure API.

Evidence:
  ../reproduced/chain-publication/
  ../reproduced/chain-publication-failure/
Each retains input catalog, command, results and helper source/lock hashes.

Limits:
This is a fixed single-command pipeline with actual parsing and inventory APIs.
It is not the production executor, a generic recipe interpreter, a COPY test,
or bootstrap/firmware/transform qualification. It does not exercise concurrent
source/cache changes, interrupted publication, supervisor death or backend I/O
failure in this combined run. The cache is privately assembled and inaccessible
to the command, but this fixture does not establish an externally mounted
read-only-cache contract. It captures Git objects, not checkout mount behavior.

Record identifies the running probe executable; it does not independently bind
the external worker, its libraries or the guest kernel in inventory fields.
Their hashes are in the separate reproduction catalog, not an attestation.
The fixed guest runtime is still part of the trusted environment. No hardware
attestation or complete environment-identity claim follows from this test.
Known ACL/metadata limitations and other ledger blockers remain unchanged.

Cancellation after private inventory finalization
-------------------------------------------------
Run the same guest.py command with case chain-publication-cancel and a new out
directory. Evidence: ../reproduced/chain-publication-cancel, including local
source snapshots. The full committed-input/restricted-command/chain path runs
first. After Artifact::finalize, the helper verifies the private deterministic
inventory, installs the actual conglobate cancellation handler and receives
SIGTERM from a trusted child targeting only its parent. cancel::check must
observe cancellation before the helper calls Artifact::publish.

Publication must return the explicit build-cancelled error. Neither the public
output directory nor any .conglobate-output-* temporary directory may remain.
The private inventory must remain unchanged. After the helper returns and its
resource guards drop, the guest requires zero device-mapper mappings and loop
attachments. The runner rejects a normal publication-success marker in this
case. The test treats the expected cancellation error as a passing assertion;
it does not propose returning success for cancelled production builds.

Scope: deterministic SIGTERM before publication starts, after workers already
stopped and inventory finalized. This is not cancellation during copy/rename,
worker setup/active execution, SIGKILL/supervisor death, or every registered
signal. Atomic-publication race semantics and the remaining teardown boundaries
remain open. Temporary private diagnostic files are not published artifacts.

The normal publication path also passed with matching shared-source hashes
(and its own case-specific init); retained in
../reproduced/chain-publication-cancel-control.
