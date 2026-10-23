# Differential second-filesystem admission test

This is a deliberately incompatible test environment, never a production policy.
The normal exact-security.selinux exception remains loaded. An additional BPF
inode_setxattr hook recognizes the production anonymous admission-probe value,
allows writes on its first observed device and denies a different device. It
preserves prior LSM errors. The fixed test program requires no conglobate hook,
CLI change, registration API or userspace map updates.

The trusted VM loader pins the test hook and a counter map; its observer reads
the map after conglobate exits. All loading/observation happens inside the
disposable VM. The host only compiles and packages inputs. Both hook source and
compiled objects are captured by the ordinary runtime input hashes.

Use `tests/distro/run.py --label-fault-second-filesystem` with:

1. `tests/distro/labeling/recipe.yaml`: require build success, allowed=1, denied=0.
2. `tests/distro/bootstrap-labeling/recipe.yaml`: require pre-command failure,
   allowed=1, denied=1, complete cleanup and no published output.

Run this directory's `verify.py RUN_DIRECTORY` (add `--negative` for bootstrap).
The existing no-fault bootstrap run supplies the positive two-root counterpart.
Removing the bootstrap output probe would make this negative build succeed,
so these assertions distinguish root-only probing from both-root probing.

The probe value is intentionally matched exactly; changing production's probe
value requires updating the fixture. Missing hooks, failed device reads or
unobserved writes cannot pass the counter assertions. This establishes the
specific admission behavior on the tested kernel; broader sandbox and hardware
qualification remain separate obligations.
