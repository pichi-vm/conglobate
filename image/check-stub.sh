#!/usr/bin/env bash
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$root/.." && pwd)"

fail() { echo "image check: $*" >&2; exit 1; }

readme="$root/README.md"
grep -q 'ghcr.io/pichi-vm/conglobate' "$readme" || fail "README must name the GHCR image"
grep -q ':latest' "$readme" || fail "README must document the :latest tag"
grep -q '0.2.0' "$readme" || fail "README must document the version tag"
grep -q 'virtiofs' "$readme" || fail "README must document virtiofs"
grep -q '## OCI output' "$readme" || fail "README must document OCI output"
grep -q 'Pichi #28' "$readme" || fail "README must document host OCI import"
grep -q 'e2fsprogs' "$readme" || fail "README must document image-resident e2fsprogs"
grep -qi 'self-host' "$readme" || fail "README must keep self-host as later"
if grep -qi 'non-promotion' "$readme"; then
	fail "README must not deny promotion"
fi

for tag in repo cache output; do
	unit="$root/units/run-pichi-${tag}.mount"
	[[ -f "$unit" ]] || fail "missing $unit"
	grep -q "^What=${tag}$" "$unit" || fail "$unit must mount tag ${tag}"
	grep -q "^Where=/run/pichi/${tag}$" "$unit" || fail "$unit Where= mismatch"
	grep -q "^Type=virtiofs$" "$unit" || fail "$unit must be Type=virtiofs"
done

service="$root/units/conglobate-build.service"
[[ -f "$service" ]] || fail "missing $service"
grep -q 'ExecStart=/usr/bin/conglobate appliance' "$service" || fail "service must ExecStart conglobate appliance"
grep -q 'Type=oneshot' "$service" || fail "service must be Type=oneshot"
grep -q 'Requires=.*run-pichi-repo.mount' "$service" || fail "service must Require repo mount"
grep -q 'Requires=.*run-pichi-cache.mount' "$service" || fail "service must Require cache mount"
grep -q 'Requires=.*run-pichi-output.mount' "$service" || fail "service must Require output mount"
grep -q '^RemainAfterExit=no$' "$service" || fail "service must not remain after exit (SuccessAction would not run)"
grep -q '^SuccessAction=poweroff-force$' "$service" || fail "service must power off on success"
grep -q '^FailureAction=poweroff-force$' "$service" || fail "service must power off on failure"
grep -q '^StandardOutput=journal+console$' "$service" || fail "service must mirror stdout to the console"
grep -q '^StandardError=journal+console$' "$service" || fail "service must mirror stderr to the console"

plan="$root/conglobate.plan.yaml"
[[ -f "$plan" ]] || fail "missing plan skeleton"
grep -qi 'self-host' "$plan" || fail "plan must stay labelled self-host"
grep -qi 'later' "$plan" || fail "plan must stay labelled later"
grep -qi 'not the bootstrap' "$plan" || fail "plan must say it is not the bootstrap builder"

conf="$root/mkosi.conf"
[[ -f "$conf" ]] || fail "missing mkosi.conf"
grep -q '^Distribution=fedora$' "$conf" || fail "mkosi.conf must be Fedora"
grep -q '^Release=43$' "$conf" || fail "mkosi.conf must pin Fedora 43"
grep -q 'e2fsprogs' "$conf" || fail "mkosi.conf must install e2fsprogs"
grep -q 'kernel-modules' "$conf" || fail "mkosi.conf must install kernel-modules for virtiofs"
grep -q 'systemd' "$conf" || fail "mkosi.conf must install systemd"
if grep -E '/usr/lib/modules/.*/(fuse|virtiofs)' "$conf"; then
	fail "mkosi.conf RemoveFiles must not drop fuse or virtiofs"
fi

[[ -f "$root/mkosi.repart/10-root.conf" ]] || fail "missing repart root"
grep -q '^Format=ext4$' "$root/mkosi.repart/10-root.conf" || fail "root partition must be ext4"
grep -q '^Minimize=guess$' "$root/mkosi.repart/10-root.conf" || fail "root must use content-based sizing"
[[ -f "$root/config.json" ]] || fail "missing config.json"

[[ -x "$root/build.sh" ]] || fail "build.sh must be executable"
grep -q 'pichi import raw' "$root/build.sh" || fail "build.sh must import the raw carapace"
grep -q 'pichi import pmi' "$root/build.sh" || fail "build.sh must import the PMI"
grep -q '/usr/bin/conglobate' "$root/build.sh" || fail "build.sh must install /usr/bin/conglobate"
grep -q 'virtiofs' "$root/build.sh" || fail "build.sh must stage virtiofs autoload"
# build.sh must not push. Promotion is the main-only index job.
if grep -q 'pichi manifest push' "$root/build.sh"; then
	fail "build.sh must not push to the registry"
fi

[[ -f "$root/mkosi.finalize" ]] || fail "missing mkosi.finalize"
grep -q 'virtiofs' "$root/mkosi.finalize" || fail "finalize must require virtiofs"
grep -q 'FUSE_FS' "$root/mkosi.finalize" || fail "finalize must require fuse"
grep -q 'mke2fs' "$root/mkosi.finalize" || fail "finalize must require mke2fs"
python3 -m py_compile "$root/mkosi.finalize"
bash -n "$root/build.sh"

workflow="$repo/.github/workflows/build.yml"
[[ -f "$workflow" ]] || fail "missing build workflow"
grep -q "github.ref == 'refs/heads/main'" "$workflow" || fail "push must be limited to main"
grep -q "github.event_name != 'pull_request'" "$workflow" || fail "pull requests must not push"
grep -q 'pichi manifest push' "$workflow" || fail "workflow must push the index on main"
grep -q ':latest' "$workflow" || fail "workflow must tag :latest"
grep -q 'CONGLOBATE_VERSION' "$workflow" || fail "workflow must tag the crate version"
grep -q '\-\-os pichi' "$workflow" || fail "workflow must set platform.os=pichi"
grep -q 'arch amd64' "$workflow" || fail "workflow must annotate amd64"
grep -q 'arch arm64' "$workflow" || fail "workflow must annotate arm64"
grep -q 'ubuntu-24.04-arm' "$workflow" || fail "workflow must build arm64 natively"
# The push lines live in the index job. The job's if: is the gate; both must exist.
index_if="$(awk '/^  index:/{p=1} p && /^    if:/{print; exit}' "$workflow")"
[[ -n "$index_if" ]] || fail "index job must have an if gate"
grep -q 'refs/heads/main' <<<"$index_if" || fail "index if must require main"
grep -q 'pull_request' <<<"$index_if" || fail "index if must exclude pull_request"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)"
grep -q "CONGLOBATE_VERSION: \"$version\"" "$workflow" || fail "workflow version must match Cargo.toml ($version)"

echo "image check: ok"
