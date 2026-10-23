# Maintained guest runtime migration

Moved a copy of the rootless GuestRuntime constructor to tests/vm_runtime.py.
Original remains unchanged. Python AST comparison proves no executable change;
the added license header is the only content addition. Lifecycle, distro and
seed runner imports were inspected in independent Python processes and all
resolve the maintained class. All three command help paths succeed.

Lifecycle and distro source archives, including seed archives, now capture the
maintained helper. The lifecycle VM source snapshot was checked against current
helper, runner and production Rust bytes and contains no historical runtime path.
A fresh disconnected native amd64 VM passes all two-scute chain assertions,
independent status and poweroff. No new distro or seed build is claimed from
help/import checks. No privileged host operations or production changes.

This removes one retirement dependency, not all historical dependencies: distro
label-environment packaging still uses separately preserved policy assets, and
large /var/tmp artifacts are not durably archived. No legacy evidence was
removed. M4 adherence and all remaining qualification/retirement gates stay open.
