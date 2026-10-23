# Remove teardown record-path plumbing

After cgroup record removal, the teardown parent path was only forwarded among
Step, RunningCommand, ObservedCommand and storage execution wrappers. Removed
that parameter, the stored Teardown.parent field, and all call-site arguments.
Cgroup creation still receives its actual parent; cancellation uses its owned
descriptors. No command quiescence, timeout, failure retention, storage ordering
or publication condition changes.

Full 227 tests and 47 doctests pass; root and maintained lifecycle strict Clippy
pass, formatting and diff whitespace checks pass. Compiler diagnostics locating
old call sites are preserved. The preceding domain-removal VM evidence covers
containment before this parameter-only cleanup; current-source hosted VM CI is
still required. No milestone adherence gate advances.
