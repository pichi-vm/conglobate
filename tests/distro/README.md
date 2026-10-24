# Distro qualification inputs

`context/recipe.yaml` is a small offline CLI regression using the Fedora seed.
The runner commits `message.txt` with the recipe; COPY installs it into the image,
and a command checks its bytes and the absence of context mounts. The runner then
requires successful publication/export and verifies exported blob digests. This
checks the committed-context pipeline, not distro boot or full qualification.

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

`seed.py` prepares an initial tool filesystem outside conglobate from an explicitly
SHA-256-pinned complete rootfs tar archive. It rejects layered whiteouts and member
path traversal, formats/extracts only in a disconnected disposable VM, checks
passwd/shadow numeric ownership and modes, unmounts, and runs e2fsck. Its sparse
64-GiB filesystem can then be converted into an initial carapace externally. This
is initial seed preparation, not a conglobate bootstrap-stage pass. The Debian
seed provenance records its OCI origin and external importer identity.

The runner accepts `--reference` to name the pinned seed in the generated lock.
For recipes that package an existing firmware pair, `--firmware-pmi` and
`--firmware-dtb` copy both files into the committed context's `firmware/` directory.
They become measured context inputs; the binary does not fetch or select firmware.
The Debian recipe currently uses the known Fedora-built kernel/corium platform
pair with Debian userspace, then the default DTB transformation binds its new root.
That tests Debian APT/systemd and firmware-stage extraction, not Debian kernel
packaging or new arma firmware generation.


The disposable VM environment now loads a fixed exact-name SELinux-label
exception before invoking conglobate. `label_environment.py` builds the trusted
fixture loader outside the binary and retains its source in each run. Loading
happens only inside the VM. `--without-label-policy` is a harness negative-test
switch; conglobate has no policy-loading option or dependency.

The trusted guest init records the build script exit status on a separate 4-KiB
virtio result device and syncs it before reboot. Recipe commands cannot access
that device. The runner requires both the exact build/export console marker and
a successful result record, then verifies all exported blob hashes. This avoids
relying on an exit line that the kernel's reboot printk can interrupt; no sleep
is used. Expected failed builds still produce a failing runner verdict, with
negative-test assessments recorded separately.

`--network-fixture` stages the maintained lifecycle fixture as a trusted DNS/TCP
service in the disposable VM and waits for pipe-based readiness. It supplies
that service's address through the ordinary `--resolv-conf` input. The service
answers only the fixed `build.fixture` question and checks the expected TCP
request; it does not fetch artifacts. The runner waits for successful service
completion and checks that no conglobate veth links or nftables tables remain
on either build success or failure. The service has a finite observation deadline.

`network/recipe.yaml` exercises actual recipe DNS/TCP, read-only resolver injection,
and a following offline step. `network/nonzero.yaml` and `network/timeout.yaml`
reach the same exchange and then fail deliberately. Assess each completed run with
`python3 tests/distro/network/verify.py RUN success|nonzero|timeout`. Failed builds
still make the runner fail; the separate assessment requires the expected error,
independent rejection status, output absence and cleanup checks. These are local
VM-destination tests, not proof of external outbound NAT or IPv6 connectivity.

`--external-network-fixture` starts a separate ordinary-user service bound only to
host loopback on an ephemeral port. The port and per-run request token are added
to the committed context as `external-endpoint`. `network/external.yaml` copies
that input through COPY, reaches the endpoint via the VM's QEMU gateway and
requires its reply. The service independently records the exchange, is bounded
by a timeout, and is terminated/reaped on runner failure. No privileged host
network configuration occurs. Assess with `network/verify.py RUN external`.
The runner also starts trusted tcpdump inside the VM before admission, waits for
its readiness, and captures only the outgoing TCP SYN to the selected fixture
port. The safe Rust observer requires the source to be the VM's 10.0.2.15 and
the destination to be 10.0.2.2 at that port, before QEMU's outer translation.
Use `network/verify.py RUN external-nat` to require this observation as well as
external reply, cleanup and independent build status. `external` assesses only
connectivity for earlier runs. Tcpdump explicitly runs as root in the trusted
VM fixture because this minimal runtime has no tcpdump account. No host packet
capture occurs. IPv6 and unsolicited inbound filtering remain separate tests.
