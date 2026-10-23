# First hosted lifecycle run

Run 40038208561 at ff2fbe7 on Ubuntu 24.04. KVM preflight, kernel selection and fixture build succeeded. cache-storage booted and passed all six checks, including kernel verity reads and corruption rejection. The next contained launch failed before guest boot: QEMU reported permission denied opening KVM. No contained/runtime-scope/clone-filter success is claimed. Raw logs/source/status/inputs retained.

A one-time ACL was therefore insufficient on this runner; the exact external cause of the permission change is not established. Workflow correction installs a runner-user-only KVM udev rule, reloads/triggers it, waits for udev completion, and records/checks device access before each case. No sleep-based synchronization or privileged host fixture execution is added. The hosted retry must establish whether this correction is sufficient.

First artifact download failed because gh's /tmp zip exceeded local quota. Retried successfully with TMPDIR on /storage. Large downloaded kernel/initramfs assets have local hashes; hosted retention is finite and no durable M8 archive is claimed. The source comparison/other platform qualification remain separate obligations.
