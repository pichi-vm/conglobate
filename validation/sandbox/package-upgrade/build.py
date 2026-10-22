#!/usr/bin/env python3
"""Build two bounded, data-only test RPMs without installing host packages."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rpmbuild", required=True, type=Path)
parser.add_argument("--out", required=True, type=Path)
args = parser.parse_args()
out = args.out.resolve()
out.mkdir()
spec = Path(__file__).resolve().parent / "fixture.spec"
binary = args.rpmbuild.resolve()
env = dict(os.environ, SOURCE_DATE_EPOCH="1700000000")
for version in ("1", "2"):
    with (out / f"build-{version}.log").open("wb") as log:
        subprocess.run([
            str(binary), "-bb", "--define", f"_topdir {out}/build-{version}",
            "--define", f"fixture_version {version}", "--define", "_buildhost fixture",
            "--define", "clamp_mtime_to_source_date_epoch 1", str(spec),
        ], stdout=log, stderr=subprocess.STDOUT, env=env, check=True)
    rpm = out / f"build-{version}/RPMS/noarch/conglobate-validation-fixture-{version}-1.noarch.rpm"
    shutil.copy2(rpm, out / f"version-{version}.rpm")
paths = [binary, spec, Path(__file__).resolve(), out / "version-1.rpm", out / "version-2.rpm"]
(out / "build-inputs.json").write_text(json.dumps(
    {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}, indent=2) + "\n")
print(f"Built both fixture versions under {out}")
