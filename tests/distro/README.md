# Distro qualification inputs

`prepare.py` copies an existing local artifact manifest and its components into
a new OCI image-layout directory, verifies their SHA-256 digests and sizes, and
creates a pinned architecture index for conglobate's lock file. It never downloads
artifacts. Run it without privileges:

```sh
python3 tests/distro/prepare.py \
  --source-cache /path/to/cache \
  --manifest sha256:MANIFEST_DIGEST \
  --architecture amd64 \
  --output /path/to/new/cache
```

The output must not exist. Successful output is made read-only and `index.pin`
contains the index digest. Failed attempts remain available for inspection.
The supplied architecture describes the existing artifact; this script does not
inspect executable machine types or convert architectures.

This is fixture preparation, not distro qualification. Conglobate must still
verify the carapace chain, execute a real distro recipe, and produce an image
that boots. The Fedora preparation recorded in
`preservation/m7-review/fedora-input-preparation.json` has only passed component
integrity and output nonreplacement checks. See `M7-QUALIFICATION.md` for the
remaining build, boot, SELinux and platform requirements.

The initial real Fedora recipe is `fedora/recipe.yaml`. It uses the existing
local Fedora carapace, runs RPM/DNF, installs SELinux tooling and targeted policy,
and labels the resulting root with that policy. It inherits firmware. A completed
build alone does not qualify booting or enforcing the installed policy.

`run.py` builds a fresh disposable amd64/KVM VM using the same trusted runtime
constructor as the integration tests. It adds outbound QEMU user networking for
package transactions; the existing regression VM remains disconnected. Supply
`--kernel`, `--kernel-release`, `--module-root`, `--cache`, `--pin`, and `--recipe`
plus a new output directory. The current lock generation uses the Fedora recipe's
`qualification.local/fedora:43` reference. All conglobate artifacts remain local;
network downloads occur only in the explicitly network-enabled DNF command.

The supervisor exports the finished OCI layout to a dedicated raw virtual disk,
which commands cannot access. The host extracts it with Python's data-only tar
filter and checks all blob digests. Source snapshots, committed context bundle,
input hashes, console logs and verdicts remain under the run directory, including
failures. Workspaces remain RAM-backed. No host mount, device-mapper operation or
BPF load is performed. The runtime constructor must move out of exploratory
validation before M8; the runner currently covers a build, not the separate boot.

`boot.py ARTIFACT NEW_OUTPUT_DIRECTORY` is an optional pichi/dillo interoperability
check of the produced image. It uses an isolated cache and loads a disposable
copy because the installed pichi loader consumes input blob paths. Neither tool
is a conglobate runtime dependency, and this does not replace the ORAS CI checks.
The default required marker comes from the Fedora recipe's oneshot service, which
checks enforcing mode, the target file's expected SELinux label and root ownership.
The harness terminates the VM after the marker; it does not claim guest shutdown.
A different `--expect` marker narrows the claim accordingly (e.g. login alone does
not execute the file-label assertions). Console logs and launcher identities are
preserved. Waiting uses pipe readiness, not synchronization sleeps.

The runner also supplies `--formatter-runtime` for bootstrap recipes. Its manifest
names the trusted environment's mke2fs executable and loader/library files already
copied into the VM. It enumerates dependencies only from the trusted host tool;
it never runs ldd on a recipe/image executable. Formatter bytes are captured and
measured by conglobate under the normal bootstrap contract.
