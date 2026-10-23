#!/usr/bin/env python3
# SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
# SPDX-License-Identifier: Apache-2.0
"""Prepare local distro fixtures without mounting filesystems or downloading data."""

import argparse
import base64
import io
import hashlib
import json
from pathlib import Path
import re


class LocalImage:
    """Copy verified components into a new cache and pin a host-platform index."""

    def __init__(self, source, manifest, architecture):
        self.source = source
        self.manifest = manifest
        self.architecture = architecture

    @staticmethod
    def identity(digest):
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
            raise ValueError("fixture requires a lowercase SHA-256 digest")
        return digest.removeprefix("sha256:")

    def source_blob(self, digest):
        return self.source / "blobs" / "sha256" / self.identity(digest)

    @staticmethod
    def encoded(document):
        return json.dumps(document, sort_keys=True, separators=(",", ":")).encode()

    def prepare(self, output):
        # A new directory preserves failed attempts and never replaces user data.
        output.mkdir()
        blobs = output / "blobs" / "sha256"
        blobs.mkdir(parents=True)
        raw = self.source_blob(self.manifest).read_bytes()
        if hashlib.sha256(raw).hexdigest() != self.identity(self.manifest):
            raise ValueError("source manifest digest mismatch")
        manifest = json.loads(raw)
        if (
            manifest.get("schemaVersion") != 2
            or manifest.get("mediaType") != "application/vnd.oci.image.manifest.v1+json"
            or manifest.get("artifactType") != "application/vnd.pichi.artifact.v1+json"
        ):
            raise ValueError("expected a pichi artifact manifest")
        for descriptor in [manifest["config"], *manifest["layers"]]:
            digest = self.identity(descriptor["digest"])
            size = descriptor["size"]
            if type(size) is not int or size < 0:
                raise ValueError("invalid component size")
            target = blobs / digest
            # Identical digests may appear more than once. Recheck their sizes.
            if target.exists():
                if target.stat().st_size != size:
                    raise ValueError("inconsistent duplicate component size")
                continue
            hash_state = hashlib.sha256()
            count = 0
            source = (io.BytesIO(base64.b64decode(descriptor["data"], validate=True))
                      if "data" in descriptor else self.source_blob(descriptor["digest"]).open("rb"))
            with source:
                with target.open("xb") as destination:
                    while block := source.read(1024 * 1024):
                        destination.write(block)
                        hash_state.update(block)
                        count += len(block)
            if count != size or hash_state.hexdigest() != digest:
                raise ValueError("source component content mismatch")
            target.chmod(0o444)
        manifest_file = blobs / self.identity(self.manifest)
        with manifest_file.open("xb") as file:
            file.write(raw)
        manifest_file.chmod(0o444)
        index = self.encoded({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [{
                "mediaType": manifest["mediaType"],
                "artifactType": manifest["artifactType"],
                "digest": self.manifest,
                "size": len(raw),
                "platform": {"os": "pichi", "architecture": self.architecture},
            }],
        })
        pin = "sha256:" + hashlib.sha256(index).hexdigest()
        (blobs / self.identity(pin)).write_bytes(index)
        (output / "index.json").write_bytes(index)
        (output / "oci-layout").write_text('{"imageLayoutVersion":"1.0.0"}\n')
        (output / "index.pin").write_text(pin + "\n")
        for path in output.rglob("*"):
            path.chmod(0o555 if path.is_dir() else 0o444)
        output.chmod(0o555)
        return pin

    @classmethod
    def main(cls):
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--source-cache", type=Path, required=True)
        parser.add_argument("--manifest", required=True)
        parser.add_argument("--architecture", choices=["amd64", "arm64"], required=True)
        parser.add_argument("--output", type=Path, required=True)
        arguments = parser.parse_args()
        print(cls(arguments.source_cache, arguments.manifest, arguments.architecture)
              .prepare(arguments.output))


if __name__ == "__main__":
    LocalImage.main()
