# M3 adherence pass 2 — clean

Frozen scope 28a108b, self-review, invalid-input/read-authority order. Streak 2/3.

Requirements and safety: reviewed confined captured-link resolution and verified
cache streams. Context resolution is entirely over the captured map; absolute,
escaping, cyclic and non-directory links fail. Git object kinds are checked as
commit/tree/blob; ancestor tree duplicates are rejected before subtree selection.
Cache::blob opens relative to pinned directory authority with BENEATH and
NO_MAGICLINKS, rejects nonregular files, and never creates/downloads inputs.
Metadata and stream reads authenticate the consumed bytes and declared lengths.
Index resolution rejects direct manifests, incompatible/ambiguous hosts and invalid
envelopes; full reference/chain verification remains a distinct pre-execution gate.

Style and failure: read-only ContextFile borrows immutable captured bytes; parsed
locators/digests are not authority handles. Cache and capture errors cannot return
partially verified runtime values. Validators are methods/conversions on domain
types; raw serde forms are separate from private invariant-bearing fields. The
M3 path acquires no mounts, processes or output token, so RAII is ordinary input
handle ownership; later kernel lifetimes are not inferred from these values.

Evidence: rechecked malformed tree/link fixtures, post-capture mutation and packed
Git support, cache replacement/truncation/escape tests, and all-stage map preflight.
Tests assert errors and stable identities, not merely method return types. Scalar,
recipe and template tests cover overflow, null/omission, strict fields, root-variable
unavailability and dollar escaping. Rootless results establish these boundaries
only. Architecture and inventory claims remain scoped to actual selected inputs.

Preservation: no changed old evidence or source retirement. The new regression and
logs are committed; current untracked files remain untouched. No finding in this
pass and no advance to privileged-lifecycle work yet.
