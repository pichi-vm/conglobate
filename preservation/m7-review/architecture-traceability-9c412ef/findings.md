# Requirements/evidence-first architecture review: findings

Read inventory schema/wire migration, validation requirements and remaining
implementation decisions in DESIGN; sandbox storage, labeling, pipeline,
network and cleanup requirements; prior M2 policy-revision pass boundaries;
current storage/step handoff and typestate source; maintained test locations.

Finding: DESIGN section 12 still required LSM authorization of both roots and
"denial elsewhere", contradicting the global exact-name exception. Section 13
still called for a multi-filesystem LSM lifecycle. Corrected both to restricted
both-root behavioral admission and confinement/read-only/attribute boundaries.
Historical scoped mechanism evidence remains accurately labeled and untouched.
Changed stale version-two implementation-status prose into a normative schema
requirement; no schema or implementation change.

Traceability improvement: architecture now maps contract groups to actual source
and tests, distinguishes current VM checks from rootless tests and historical
noncompiling policy fixtures, and explicitly leaves broad fault migration,
distro/platform coverage and artifact recovery outstanding. This is architecture
traceability, not acceptance of those implementations.

Ownership/style observations: Volume state traits are sealed, Mounted retains
root/lease, views borrow the owner, namespace-sensitive views are thread-affine.
ExecutionStorage handoff derives both bootstrap descriptors from retained owners.
Teardown remains fallible before storage finalization; no policy owner is needed.
These observations complement earlier review but do not replace full passes.

Result: findings, 0/3. No source edits, privileged operations or evidence removal.
Corrected document hashes in scope.json; historical versions remain in Git.
