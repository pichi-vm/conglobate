Static trusted-runtime identity audit (2026-10-20)
==================================================
Reproduction:
  python3 validation/sandbox/runtime-identity/audit.py \
    /tmp/conglobate-runtime-link-proc-v1 --out /tmp/NEW-runtime-identity.json
  python3 validation/sandbox/runtime-identity/check-audit.py

Inspected the retained packaged filesystem from the actual FUSE proc-symlink
rejection test. This is a read-only static inspection, not another guest run.
No fixture binary executed: host readelf inspected ELF program/dynamic headers.
98 recorded files and 5 symlinks match; no unlisted root files. 65 ELF files and
5 shebang scripts were found. All requested ELF interpreters exist inside the
fixture. All DT_NEEDED entries have packaged basename candidates. No inspected
ELF has DT_RPATH or DT_RUNPATH. This does not emulate loader search or prove ABI
compatibility. Included files are not necessarily all reachable by the worker.
The audit's owned text controls detect changed bytes, changed symlink targets and
unlisted files; the unchanged control passes.

entry-helper, runtime-mounts and publication-helper dynamically depend on libc,
libgcc_s and the loader. The patched worker declares libfuse3.so.4, libblkid,
libuuid and libc. Bash declares libtinfo and libc. The packaged /etc contains
passwd, group, nsswitch.conf and an empty resolv.conf. ld.so.preload, ld.so.cache
and ld.so.conf are absent. report.json preserves the per-file edges and hashes.

Source findings and implications
--------------------------------
1. guest.py records file hashes and symlink targets, but not mode, UID/GID,
   xattrs or directory permissions. cpio normalizes owners to 0:0 but this is
   separate packaging behavior; the JSON inventory alone does not bind metadata.
   This inventory is test evidence, not the final build Record or attestation.
2. Libraries are discovered using host ldd. It inherits the host environment
   (some cases override LD_LIBRARY_PATH) and only checks stdout for 'not found';
   it does not generally require successful ldd exit. That is acceptable only
   within this controlled fixture assumption, not a hermetic runtime resolver.
3. chain.rs Command::new uses inherited environment and a launcher selected via
   a trusted diagnostic variable or PATH. The worker/setup bash execute before
   entry-helper supplies the final explicit PATH/HOME environment. Sanitization
   must happen before trusted child exec, not just before the image command.
   This is not evidence that recipe commands can mutate the supervisor env.
4. DT_NEEDED omits dlopen/NSS/plugins, subprocesses and interpreted imports.
   Package identity needs an immutable complete trusted runtime tree (including
   metadata/configuration), or an equivalently validated dependency closure.
   The binary need not own appliance packaging, but the trusted execution and
   eventual attestation contract must identify the runtime it actually uses.
5. Runtime immutability and environment admission remain unqualified. Matching
   retained source/package bytes does not establish either property. Remaining
   dynamic-load and runtime-binding gates stay open in areas 3/19/21.

Source snapshots and report hashes are retained with this evidence. No production
executor change, security certification or backend adoption is claimed.
