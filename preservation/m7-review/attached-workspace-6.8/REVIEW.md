# Attached workspace compatibility probe

The maintained workspace-mount case passes in a disposable native amd64 guest
using Ubuntu 6.8.0-147-generic from failed CI run 40039132104. The modules were
extracted from that run's trusted guest initramfs; the probe uses the local
userspace tools recorded in inputs.json. It is not the identical CI userspace.

The detached source first rejects OPEN_TREE_CLONE with EINVAL. The same tmpfs,
after attachment under a protected directory, supplies a directory subtree and
a resolver-file view. Both preserve bytes and reject write opens with EROFS.
Normal unmount rejects the live mount descriptor with EBUSY, then succeeds after
its release; the empty attachment directory is removed. Guest exits zero and
powers off. No lazy unmount, host privileged operation or command execution is
used. The probe does not run the production Workspace API and proves neither
complete command confinement nor failure cleanup. Those remain required before
adopting attached storage in production.

This source archive predates the subsequent removal of unused domain recovery
entry points; it includes the uncommitted probe source captured by the runner.
Raw source/logs/results are retained. Large guest assets remain local with hashes,
not durably archived. No milestone adherence pass is awarded.
