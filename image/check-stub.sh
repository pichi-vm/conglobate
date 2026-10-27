#!/usr/bin/env bash
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0
# Validate the image/ stub contract without building or booting a VM.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"

fail() { echo "image stub check: $*" >&2; exit 1; }

readme="$root/README.md"
grep -qi 'stub' "$readme" || fail "README.md must label this tree as a stub"
grep -qi 'non-promotion\|Follow-up A\|not claimed' "$readme" || fail "README.md must deny promotion"

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

recipe="$root/conglobate.recipe.yaml"
[[ -f "$recipe" ]] || fail "missing recipe skeleton"
grep -qi 'Follow-up A\|Skeleton only\|stub' "$recipe" || fail "recipe must stay labelled as stub/skeleton"

echo "image stub check: ok"
