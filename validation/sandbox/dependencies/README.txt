Prototype dependency and safe-API inventory
==========================================
Reproduce (all inputs are trusted provisioned binaries/source):
  python3 validation/sandbox/dependencies/inventory.py \
    --worker /tmp/NEW-WORKERS/acl-patched \
    --e2fs-source /path/to/e2fsprogs-1.47.3 \
    --fuse-source /path/to/fuse-3.18.3 --out /tmp/NEW-DEPENDENCIES
The script executes ldd on the supplied trusted worker; do not use untrusted
image executables. It installs nothing and runs no image command.

Evidence: ../reproduced/dependencies/{inventory.json,ldd.txt,checks.txt}.
The Cargo graph is resolved offline with --locked. The report includes enabled
features, dependency edges, manifest/lock hashes, local license-file hashes,
compiler/tool versions, worker/library hashes and owning RPM license labels.

Entry helper's resolved Cargo graph (declared licenses):
  conglobate-entry-probe 0.0.0  Apache-2.0
  nix 0.30.1                   MIT
  caps 0.5.6                   MIT/Apache-2.0 (legacy declaration retained)
  seccompiler 0.5.0            Apache-2.0 OR BSD-3-Clause
  libc 0.2.191                 MIT OR Apache-2.0
  bitflags 2.13.2              MIT OR Apache-2.0
  cfg-if 1.0.5                 MIT OR Apache-2.0
  cfg_aliases 0.2.2            MIT

Safe interface boundary:
The entry crate forbids unsafe code and passes clippy with warnings denied.
It uses nix's safe chdir/pivot_root/umount2/close/execve/prctl interfaces,
caps' safe read/drop/clear/set, and seccompiler's safe filter construction and
apply_filter. The libc re-export supplies syscall numbers, not direct unsafe
syscall invocations in this helper. The crates internally invoke native APIs;
forbid(unsafe_code) is not transitive and is not a soundness audit. The current
FD enumeration/close sequence assumes this helper is single-threaded. No new
production unsafe-code exception is introduced.

Native worker boundary:
The experimental fuse2fs worker is an external C executable. Upstream e2fsprogs
NOTICE declares GPLv2 for utilities and LGPLv2 for libext2fs/libe2p, with other
library exceptions. libfuse LICENSE distinguishes its include/lib LGPL2.1
files from GPLv2 utility code. Local notice/source hashes are recorded.
The dynamic closure has libfuse3, libacl, libblkid, libuuid, libc and the loader.
RPM versions/declared labels are recorded individually. glibc's RPM license
expression describes its package; do not interpret every term as applying to
every linked object. The worker also contains statically built e2fs libraries,
so ldd alone is not a complete worker-source inventory.

These are factual license declarations and interface observations, not a
license-compatibility or distribution approval. No worker code is linked into
the Apache Rust entry helper. Shipping patched native binaries needs a
source/notice/patch inventory for the chosen release. The current build records
remain separate, and the source path passed to this script is not proof that
it produced a given binary. We have not adopted these patches as production.

Open: final backend selection (including known functional blockers), complete
appliance/package and production-binary dependency coverage, native-source
closure, maintenance/update policy, dependency soundness review and representative
performance. This advances item21's bounded prototype inventory only.

Static link closure and byte-identical relink
-------------------------------------------
Reproduce against the trusted retained paired build (installs nothing):
  python3 validation/sandbox/dependencies/native_closure.py \
    --pair /tmp/conglobate-acl-protocol-v5 --out /tmp/NEW-NATIVE-CLOSURE
The output must be new. make -n recovers the generated worker link command;
the script parses it into arguments rather than executing shell text. It links
to a new output with a GNU link map and trace. The original worker/build objects
are not overwritten. This executes trusted build tools and is not a facility
for running untrusted image build scripts.

The relink is byte-identical to the tested paired worker:
9f2a0e2bdd9eb677631253326a2a3b0d530414a4c43670e2f441bd6a444421c8.
There are29 linker input files, including compiler startup objects, scripts and
libraries. Trace presence does not mean a library contributed code or will be
loaded at runtime. The map identifies61 extracted static members:57 libext2fs,
3 libcom_err,1 libe2p. libsupport is on the command but supplies no member in
this build. Four direct objects are fuse2fs, journal, recovery and revoke.
Each input/archive member is hashed; external inputs have owning RPM version
and declared-license labels. Compiler/linker/archiver versions and binaries are
also identified. Source-tree manifests record2323 worker-source entries and361
libfuse-source entries, including their notices, along with configuration hashes.

Evidence: ../reproduced/native-link-closure. No native binaries are retained in
the repository. The existing dynamic inventory remains separately relevant:
linker inputs are not the same as the runtime loader closure. This closes the
ldd-only visibility gap for the retained worker's static link membership.
It does not prove the original objects were compiled from the current source
bytes, enumerate their historical preprocessor/system-header inputs, establish
a reproducible clean build or approve distribution. Full source-tree hashes
include unused code and are conservative snapshot identities. Native maintenance,
patch/API ownership, full appliance dependency binding and source-to-object
provenance remain open. The original worker's metadata limitations are unchanged.

Source-to-object reproduction of the native pair
------------------------------------------------
Run against the retained trusted build paths and compiler environment:
  python3 validation/sandbox/dependencies/recompile.py \
    --inventory /tmp/conglobate-native-closure-v2/inventory.json \
    --pair /tmp/conglobate-acl-protocol-v5 --out /tmp/NEW-WORKER-OBJECTS
  python3 validation/sandbox/dependencies/recompile_fuse.py \
    --pair /tmp/conglobate-acl-protocol-v5 --out /tmp/NEW-FUSE-OBJECTS
Both outputs must be new. These run trusted make dry-run/compiler commands,
never image commands. Each compiler invocation is bounded to30 seconds and
writes new output files; original objects, archives and libraries are preserved.

All65 contributing worker objects reproduce exactly: four direct objects and
61 static members selected by the preceding link map. Each existing build
object first matches its recorded archive-member/direct-object hash. DWARF
identifies its source; the generated Makefile supplies its compile command.
The fresh compile records full source/header dependency hashes, including system
headers:338 unique inputs. Final rehash verifies those inputs remained unchanged.
This is positive evidence of a source/object relationship, not only a claim
based on source files being adjacent to an already-built binary.

All16 libfuse objects from compile_commands.json also reproduce exactly, with
256 unique source/header inputs. A fresh link using those rebuilt objects
produces a library byte-identical to the copied paired libfuse3.so.4. Its version
script and traced link inputs are hashed. Across both components,81 native
objects are reproduced. The worker itself already has a byte-identical relink
record; its contributing object bytes are now independently reproduced.

Evidence: ../reproduced/native-source-reproduction, with per-object compiler
commands, dependency files/hashes, original/rebuilt object hashes, summaries and
libfuse link map/trace. Large object/library binaries stay outside the repository.
Identical bytes mean the existing behavioral evidence still identifies these
objects/library; this does not broaden what those behavioral tests covered.

Limits: existing generated source/configuration and original compile working
paths are inputs. Configure/code generators were not rerun from pristine release
archives. Existing compiler, assembler, startup objects and system headers are
trusted; this is not compiler bootstrapping, historical-build attestation or
cross-toolchain reproducibility. Dependency catalogs identify header bytes but
do not independently provision them. A clean/package-managed build and a final
runtime inventory/maintenance process remain to be established. The metadata
compatibility blockers and experimental API ownership are unchanged.

Clean native pair build and behavioral regression (2026-10-20)
==============================================================
Reproduce with provisioned trusted archives, patches and build tools:
  python3 validation/sandbox/dependencies/clean_pair.py \
    --archives /tmp/conglobate-acl-source \
    --finalize /tmp/conglobate-finalize-workers \
    --pair /tmp/conglobate-acl-protocol-v5 \
    --meson-pythonpath /tmp/conglobate-fuse-protocol-tools \
    --out /tmp/NEW-CLEAN-PAIR
Then run guest.py acl-protocol with --worker-build /tmp/NEW-CLEAN-PAIR;
run guest.py metadata with --worker-build /tmp/conglobate-finalize-workers and
--protocol-worker-build /tmp/NEW-CLEAN-PAIR. Use new --out directories, --run
and --kernel-version 7.2.10-200.fc44.x86_64 for each guest.

Sources are fresh e2fsprogs1.47.3 and libfuse3.18.3 release archives from the
retained Fedora SRPM extraction. Their SHA256 and the three applied patches are
recorded. git apply --check/apply succeeded without patch rewriting. No previous
build directories, objects, generated configuration headers or libraries were
copied. configure and lib/config.h.in were deleted and regenerated with autoconf
and autoheader. Both differ from release-generated bytes; hashes are retained.
Fresh Meson and configure executions generated the build inputs; ninja and make
built the paired library and worker. Nothing installed on the host.

The modified worker source and all five modified libfuse files match the earlier
paired prototype. New binaries are NOT claimed byte-identical: build paths and
regenerated inputs differ. Both guest input manifests explicitly match the new
worker/library hashes. Six configurations (kernel ext4, initial FUSE, mapped
FUSE, each with/without metadata_csum) pass ACL common semantics, both SGID
capability-sensitive controls, fsck and kernel-remount comparisons. The metadata
matrix also completes all six configurations, exact persisted comparisons and
previously specified extent/capability tests. In both mapped configurations,
device creation, trusted xattr writes, setting restrictive flags and clearing
inherited restrictive flags still fail EPERM. Those are compatibility blockers,
not successful emulation of root metadata semantics.

The initial preparation attempt found no host patch executable before applying
anything. git apply was used instead; the retained clean_pair.py reproduces the
successful fresh-directory build. No failed binary or stale object was reused.

Scope: closes reuse of old generated build inputs for this prototype pair.
Shipped aclocal.m4, auxiliary scripts and other release source are still trusted
archive inputs. Host compiler/assembler/linker, headers, tools and Meson modules
are provisioned, not independently bootstrapped. Tool/module hashes, commands,
regeneration hashes, build logs and both guest source/input/result catalogs are
retained. This is not a hermetic toolchain, complete software supply-chain proof,
maintained backend adoption or a resolution of the mapped-VFS restrictions.
