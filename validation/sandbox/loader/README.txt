Trusted helper launch environment and a harmless loader canary
=============================================================

Reproduce:
  python3 validation/sandbox/guest.py loader \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --out /tmp/NEW-LOADER-GUEST --run

The guest has no host shares/disks/NICs. The only canary action is writing one
fixed marker to /work/loader-canary when a shared library loads. It does not
read input, alter credentials, access external resources or persist outside
the disposable guest. C is used solely for loader instrumentation; production
conglobate and the entry helper retain their forbid(unsafe_code) rule.

A trusted Python invoker supplies the environment directly to execve of the
trusted safe-Rust entry helper. Thus no preceding shell/interpreter receives
the diagnostic LD_PRELOAD. Both cases use mapped root and the helper's current
--restricted policy, root pivot/detach, descriptor closure and command exec.

Negative control:
  Inherited LD_PRELOAD loads the canary before the helper performs isolation.
  The outer marker appears even though the final image command correctly sees
  a clean environment, detached old root and closed FD9.

Sanitized case:
  The invoker constructs the helper's environment from PATH/HOME only. No outer
  marker is created. The image command again sees the expected PATH/HOME,
  no loader/library/shell/Python/conversion-path variables, no old-root path and
  closed FD9. The invoker asserts FD9 exists immediately before helper exec.

The intentionally named /usr/bin/python3 in the image is a shell-script entry
canary, as in the original entry-ordering fixture. It never performs isolation;
all isolation precedes its execution. Expected "Bad file descriptor" messages
are the positive closure assertion, not failing tests.

Evidence:
  ../reproduced/loader/{inputs.json,command.json,results.txt,source-inputs.json}
  Packaged shared library and entry-helper executable hashes are included.

Primary mechanism reference:
https://man7.org/linux/man-pages/man8/ld.so.8.html
The loader prepares a dynamically linked program before executing it, and
LD_PRELOAD selects additional objects. Clearing environment inside that program
is therefore too late to control its own startup loading.

Design implication and limits:
- Construct trusted helper/worker environments before exec; do not inherit
  recipe/environment-controlled loader settings across this boundary.
- Image commands may have their own authored environment AFTER isolation. This
  test does not propose banning image-local loader settings as recipe features.
- The helper executable, interpreter, libraries and loader configuration must
  themselves remain trusted/immutable. This test does not cover replacement of
  those files, every loader mode, LD_AUDIT, alternate libc/ABI or all environment
  variables. Only LD_PRELOAD has an executed canary; other listed variables
  are checked for absence at image entry.
- No production launcher change or complete security verdict is implied.
