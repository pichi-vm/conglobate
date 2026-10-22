#!/usr/bin/env python3
"""Fetch Debian stable UML/BusyBox using signed archive metadata, without installing."""
import argparse
import hashlib
import json
import lzma
from pathlib import Path
import subprocess
import urllib.request


def fetch(url, path):
    with urllib.request.urlopen(url, timeout=45) as response, path.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            output.write(chunk)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--keyring", default="/usr/share/keyrings/debian-archive-keyring.pgp")
    args = parser.parse_args()
    args.out.mkdir(exist_ok=False)
    base = "https://deb.debian.org/debian/"
    fetch(base + "dists/trixie/InRelease", args.out / "InRelease")
    result = subprocess.run(["gpgv", "--keyring", args.keyring, "--output",
                             str(args.out / "Release"), str(args.out / "InRelease")],
                            capture_output=True, text=True)
    (args.out / "signature.txt").write_text(result.stdout + result.stderr)
    result.check_returncode()
    index = "main/binary-amd64/Packages.xz"
    section = (args.out / "Release").read_text().split("SHA256:\n", 1)[1]
    row = next(line.split() for line in section.splitlines()
               if line.strip().endswith(" " + index))
    fetch(base + "dists/trixie/" + index, args.out / "Packages.xz")
    data = (args.out / "Packages.xz").read_bytes()
    if len(data) != int(row[1]) or hashlib.sha256(data).hexdigest() != row[0]:
        raise RuntimeError("Package index integrity mismatch")
    index_text = lzma.decompress(data).decode()
    chosen = []
    for name in ["user-mode-linux", "busybox-static"]:
        stanza = next(s for s in index_text.split("\n\n") if s.startswith("Package: " + name + "\n"))
        fields = dict(line.split(": ", 1) for line in stanza.splitlines()
                      if ": " in line and not line.startswith(" "))
        destination = args.out / Path(fields["Filename"]).name
        fetch(base + fields["Filename"], destination)
        data = destination.read_bytes()
        if len(data) != int(fields["Size"]) or hashlib.sha256(data).hexdigest() != fields["SHA256"]:
            raise RuntimeError(f"Package integrity mismatch: {name}")
        chosen.append(fields)
        print(name, fields["Version"])
    (args.out / "packages.json").write_text(json.dumps(chosen, indent=2) + "\n")


if __name__ == "__main__":
    main()
