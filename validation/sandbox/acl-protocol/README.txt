ACL SGID semantics using the kernel's protocol decision
======================================================

This is a disposable paired libfuse/worker interface experiment. It does not
install libraries, modify the guest kernel, relax command capabilities or
adopt a production backend. It extends the finalize + acl-fix prototype.

Reproduce:
1. Prepare the finalization worker source and apply acl-fix/build.py as described
   in their READMEs. The resulting source tree must contain both corrections.
2. Provide libfuse3.18.3 source, ninja, C toolchain and meson1.10.0. This run used
   pip --target /tmp/conglobate-fuse-protocol-tools meson==1.10.0; host packages
   were unchanged. The Meson Python module directory is an explicit input.
3. Run:
   python3 validation/sandbox/acl-protocol/build.py \
     --fuse-source /path/to/fuse-3.18.3 \
     --worker-source /tmp/FINALIZE-AND-ACL/source \
     --meson-pythonpath /path/to/meson-module-directory --out /tmp/NEW-PAIR
4. Run:
   python3 validation/sandbox/guest.py acl-protocol \
     --kernel-version 7.2.10-200.fc44.x86_64 \
     --worker-build /tmp/NEW-PAIR --out /tmp/NEW-ACL-GUEST --run

Mechanism:
The kernel's extended SETXATTR request contains FUSE_SETXATTR_ACL_KILL_SGID.
Stock libfuse3.18.3 omits it at the callback boundary and does not enable that
extension by default. The experiment stores the negotiated flag in the private
request object and exposes a prototype-only high-level accessor during the
callback. It leaves public context layouts unchanged. Request allocation is
zero-initialized, and the flag is populated only for an extended setxattr.

The worker requests FUSE_CAP_SETXATTR_EXT and uses the kernel flag when setting
an access ACL. It no longer substitutes UID0/group heuristics for this decision.
No /proc capability inspection or extra command privilege is needed. The
kernel remains the authority deciding whether SGID must be cleared.

The new accessor has a FUSE_PROBE_1.0 symbol version. A stock-library control
fails to load the worker; the matched library runs its --version command.
The guest packager resolves/copies the paired library explicitly. Do not use
this private symbol name as an adopted public API or mix stock/patched builds.

Results:
The same differential matrix from acl-fix runs kernel ext4, initial-userns FUSE
and mapped-userns FUSE with/without metadata_csum. All six cases now preserve
SGID for the nonroot caller with CAP_FSETID and clear it for root without that
capability, matching kernel ext4. The original fixture verifies effective
capabilities before each operation. No limitation marker is accepted in this
case; the runner requires six of each correct result.

Common access, chmod/mask, inheritance, malformed-ACL, owner/group, link/rename
and removal checks still pass. All six images pass fsck and kernel remount;
FUSE reports match the kernel reference. This corrects the two demonstrated
capability mismatches in the prototype, not every possible ACL behavior.

Evidence: ../reproduced/acl-protocol includes source-bound guest records,
original/modified input hashes, complete protocol.patch, build logs, builder
snapshot and pairing controls. Reverse-applying the patch with --check succeeds
against the built source trees. External full upstream sources, toolchains and
binaries remain provisioned inputs; hashes/logs are not hardware attestation.

Adoption gates still open:
- A maintained/upstream API and version/negotiation contract, including behavior
  without the extension; no production implementation is selected here.
- Native patch review, concurrent/failure behavior, broader workload and runtime
  qualification. Existing nontransactional/backend error paths are separate.
- Device-node, trusted.* and restrictive-file-flag kernel permission boundaries.
- Native-library/source distribution and maintenance obligations. The library
  and worker retain their upstream license boundaries; Rust remains safe-only.

Unsupported-extension negotiation and worker exit status
--------------------------------------------------------
unsupported.py copies a finished paired-library source tree and builds a
fault-injection library that removes SETXATTR_EXT from the available capability
mask immediately before initialization negotiation. It does not forge kernel
messages, change the guest kernel or grant privileges. The unchanged worker
still requests the extension. Reproduce:
  python3 validation/sandbox/acl-protocol/unsupported.py \
    --pair-build /tmp/NEW-PAIR --meson-pythonpath /path/to/meson-module-directory \
    --out /tmp/NEW-UNSUPPORTED
  python3 validation/sandbox/guest.py acl-protocol-unavailable \
    --kernel-version 7.2.10-200.fc44.x86_64 \
    --worker-build /tmp/NEW-UNSUPPORTED --out /tmp/NEW-UNSUPPORTED-GUEST --run

This exposed a separate false-success defect: libfuse reports the unsupported
want bit (0x08000000), exits its loop and returns status8 through fuse_main.
fuse2fs's default status-mapping branch converted that error to0. The initial
run fails the test and is retained in reproduced/acl-unsupported-negative.
The experimental builder now maps that nonzero/default branch to failure32.
The corrected guest requires the exact negotiation diagnostic, status32, no
mounted view and no remaining FUSE connection. It passes; the complete normal
six-case ACL matrix still passes with the same correction.

Evidence: ../reproduced/acl-unsupported and
../reproduced/acl-protocol-status-control. This is simulated feature absence,
not a run on an older kernel. It validates one initialization error and normal
operation, not every libfuse return path. The misleading early 'mounted' log
also must not be used as a readiness handshake. Production adoption and the
remaining failure/cancellation matrix stay open.
