# Current two-root bootstrap runtime evidence

Base 7730732, no application or fixture edits. Maintained tests/distro/bootstrap-labeling/recipe.yaml executed with tests/distro/run.py in a native amd64 KVM disposable guest, kernel 7.2.10-200.fc44.x86_64, 8 GiB. Exact VM command and inputs retained. Cache /var/tmp/conglobate-fedora-input-v1 pinned to sha256:4b1b5a79839b8d115987aefb23dabe4282713396101b4c5416d17be974b2b2ab; module-root /. Compiler environment CARGO_BUILD_JOBS=2, TMPDIR=/var/tmp/conglobate-rewrite-tmp, CARGO_TARGET_DIR=target/m7-stable. Local run: /storage/conglobate-validation/m4-bootstrap-runtime-current-v1.

Runner and independent bootstrap-labeling/verify.py both exit 0. Two real command steps exercise tool/output mounts; second verifies data, UID/GID 123:456 and mode 0640 after remount. Trusted formatting and production restricted-label admission execute. Runtime::scoped returns success only after explicit runtime close; context close precedes publication. No independent exhaustive resource enumeration is claimed.

Verifier confirms SHA256/size of exported descriptors, exactly one produced scute, no tool PMI/DTB, canonical inventory v2, expected geometry/label, one measured formatter runtime receipt and no networking steps. This is the minimal filesystem lifecycle fixture, not a bootable distro, arbitrary metadata fidelity or confidential-platform qualification.

Compared 187 archived source/test/Cargo files: 0 differences. Full source/context/raw status/serial and small OCI metadata preserved. Kernel/initramfs/scute remain local with hashes, not remotely durable; exported tar backing is not treated as a separate authoritative output. No full M4/M5 acceptance pass awarded.
