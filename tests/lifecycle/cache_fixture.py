# SPDX-License-Identifier: Apache-2.0
"""Small synthetic scute data; not filesystem or boot qualification."""
import hashlib
import json
import platform
import subprocess
import struct


class CacheFixture:
    def __init__(self, root):
        self.root = root
        (root / 'blobs/sha256').mkdir(parents=True, exist_ok=True)

    def blob(self, data):
        digest = hashlib.sha256(data).hexdigest()
        (self.root / 'blobs/sha256' / digest).write_bytes(data)
        return 'sha256:' + digest

    def create(self):
        for name, count, compressed in [('aligned', 8192, False),
                                         ('partial', 4097, False),
                                         ('compressed', 4097, True)]:
            data = bytes([42]) * count
            padded = data.ljust(8192, b'\0')
            salt = bytes(32)
            leaves = b''.join(hashlib.sha256(salt + padded[i:i+4096]).digest()
                              for i in (0, 4096))
            root = hashlib.sha256(salt + leaves.ljust(4096, b'\0')).hexdigest()
            blob = subprocess.check_output(['zstd', '-q', '-c'], input=data) if compressed else data
            layer = {'mediaType': 'application/vnd.pichi.scute.v1' + ('+zstd' if compressed else ''),
                     'digest': self.blob(blob), 'size': len(blob),
                     'annotations': {'dev.pichi.scute.verity.salt': salt.hex(),
                                     'dev.pichi.scute.verity.data-blocks': str((len(data) + 4095) // 4096)}}
            self.image(name, [layer], root)
        self.pinned_manifest()
        self.chains()

    def pinned_manifest(self):
        """A real content-addressed manifest for payload-mutation checks."""
        data = bytes([43]) * 8192
        salt = bytes(32)
        leaves = b''.join(hashlib.sha256(salt + data[i:i + 4096]).digest()
                          for i in (0, 4096))
        root = hashlib.sha256(salt + leaves.ljust(4096, b'\0')).hexdigest()
        layer = {'mediaType': 'application/vnd.pichi.scute.v1',
                 'digest': self.blob(data), 'size': len(data),
                 'annotations': {'dev.pichi.scute.verity.salt': salt.hex(),
                                 'dev.pichi.scute.verity.data-blocks': str((len(data) + 4095) // 4096)}}
        self.image('pinned', [layer], root)

    def image(self, name, layers, root):
        # Tip derived from scute graph; never stamp verity.hash.
        _ = root
        annotations = {'dev.pichi.carapace.verity.' + k: v for k, v in
                       [('algo', 'sha256'), ('data-block-size', '4096'),
                        ('hash-block-size', '4096'), ('version', '1'), ('hash-type', '1')]}
        manifest = json.dumps({'schemaVersion': 2,
            'mediaType': 'application/vnd.oci.image.manifest.v1+json',
            'artifactType': 'application/vnd.pichi.artifact.v1+json',
            'config': {'mediaType': 'application/vnd.oci.empty.v1+json',
                       'digest': self.blob(b'{}'), 'size': 2, 'data': 'e30='},
            'layers': layers, 'annotations': annotations}).encode()
        descriptor_digest = self.blob(manifest)
        descriptor_size = len(manifest)
        descriptor = {'mediaType': 'application/vnd.oci.image.manifest.v1+json',
                      'artifactType': 'application/vnd.pichi.artifact.v1+json',
                      'digest': descriptor_digest, 'size': descriptor_size,
                      'platform': {'os': 'pichi', 'architecture': {'x86_64': 'amd64', 'aarch64': 'arm64'}[platform.machine()]}}
        index = json.dumps({'schemaVersion': 2,
                            'mediaType': 'application/vnd.oci.image.index.v1+json',
                            'manifests': [descriptor]}).encode()
        (self.root / (name + '.pin')).write_text(self.blob(index))

    def chains(self):
        for valid in (True, False):
            parent = bytes(32)
            layers = []
            for index, value in enumerate((65, 66)):
                header = struct.pack('<IIII', 0x70416e53, int(valid or index == 0), 1, 8)
                metadata = struct.pack('<QQ', 0, 2)
                payload = bytes([value]) * 4096
                if index == 0:
                    metadata += struct.pack('<QQ', 1, 3)
                    payload += bytes([67]) * 4096
                data = header.ljust(4096, b'\0') + metadata.ljust(4096, b'\0') + payload
                leaves = b''.join(hashlib.sha256(parent + data[i:i+4096]).digest()
                                  for i in range(0, len(data), 4096))
                root = hashlib.sha256(parent + leaves.ljust(4096, b'\0')).digest()
                layers.append({'mediaType': 'application/vnd.pichi.scute.v1',
                               'digest': self.blob(data), 'size': len(data),
                               'annotations': {'dev.pichi.scute.verity.salt': parent.hex(),
                                               'dev.pichi.scute.verity.data-blocks': str(len(data) // 4096)}})
                parent = root
            self.image('chain' if valid else 'invalid-header', layers, parent.hex())
