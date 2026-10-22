#!/usr/bin/env python3
"""Verify a modified disposable RPM copy is rejected before fixture creation."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rpms", required=True, type=Path)
parser.add_argument("--kernel-version", required=True)
args = parser.parse_args()
here = Path(__file__).resolve().parent
catalog = json.loads((here / "rpms.json").read_text())
with tempfile.TemporaryDirectory(prefix="conglobate-rpm-input-check-") as temporary:
    root = Path(temporary)
    packages = root / "rpms"
    packages.mkdir()
    for name in catalog:
        (packages / name).symlink_to(args.rpms.resolve() / name)
    first = packages / next(iter(catalog))
    first.unlink()
    shutil.copy2(args.rpms / first.name, first)
    with first.open("r+b") as stream:
        byte = stream.read(1)
        stream.seek(0)
        stream.write(bytes([byte[0] ^ 1]))
    result = subprocess.run([
        "python3", str(here / "build.py"), "--kernel-version", args.kernel_version,
        "--rpms", str(packages), "--out", str(root / "rejected"),
    ], capture_output=True, text=True)
    assert result.returncode != 0 and "RPM digest mismatch" in result.stderr
    assert not (root / "rejected").exists()
print("MODIFIED_RPM_REJECTED_BEFORE_OUTPUT_CREATION")
