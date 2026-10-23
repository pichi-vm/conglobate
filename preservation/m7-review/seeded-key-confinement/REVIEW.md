# Supervisor key visibility control

The confinement fixture creates a benign user key in the disposable VM root's
user keyring before invoking conglobate. The trusted control requires its sentinel
description to appear in /proc/keys. A second trusted control after the complete
build requires it to remain visible, so an empty command view cannot be explained
by the fixture key simply disappearing when the first control exits.

Inside the production command sandbox, the existing probe requires /proc/keys
to be empty or inaccessible. The same run retains ordinary proc/PID/mountinfo
checks, hidden kernel addresses, unavailable sched_debug, descriptor/capability
checks, syscall denials, allowed sockets/processes, fresh runtime mounts and
live-descendant cleanup. All verifier checks passed, including output publication
and export. Source comparison records matching application/fixture source; only
the explanatory confinement README was updated after the source snapshot.

This is a benign fixture, not an exploit: only an explicitly created VM-owned key
is checked; no unrelated payloads or key metadata are printed. No namespace or
policy operations run on the host. The key lives until disposable VM shutdown.
No production Rust was changed. Probe compilation and Python syntax checks pass;
the native amd64 VM exercises the full maintained confinement recipe.

security-003 remains partially qualified: sched_debug-present kernel coverage and
arm64 execution remain outstanding. This is not a complete M4 adherence pass,
general security proof, or confidential-platform qualification. Small raw evidence
and source/context archives are retained. Large VM/output assets and artifact
blobs are hashed at local paths, not claimed remotely durable.
