"""Small synthetic scute data; not filesystem or boot qualification."""
import hashlib
import json
import subprocess


class CacheFixture:
    def __init__(self, root):
        self.root = root
        (root / 'blobs/sha256').mkdir(parents=True)

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
                     'annotations': {'dev.pichi.scute.verity.salt': salt.hex()}}
            annotations = {'dev.pichi.carapace.verity.' + k: v for k, v in
                           [('algo', 'sha256'), ('data-block-size', '4096'),
                            ('hash-block-size', '4096'), ('version', '1'), ('hash-type', '1'),
                            ('hash', root)]}
            manifest = json.dumps({'schemaVersion': 2,
                'mediaType': 'application/vnd.oci.image.manifest.v1+json',
                'artifactType': 'application/vnd.pichi.artifact.v1+json',
                'config': {'mediaType': 'application/vnd.oci.empty.v1+json',
                           'digest': self.blob(b'{}'), 'size': 2, 'data': 'e30='},
                'layers': [layer], 'annotations': annotations}).encode()
            descriptor = {'mediaType': 'application/vnd.oci.image.manifest.v1+json',
                          'artifactType': 'application/vnd.pichi.artifact.v1+json',
                          'digest': self.blob(manifest), 'size': len(manifest),
                          'platform': {'os': 'pichi', 'architecture': 'amd64'}}
            index = json.dumps({'schemaVersion': 2,
                                'mediaType': 'application/vnd.oci.image.index.v1+json',
                                'manifests': [descriptor]}).encode()
            (self.root / (name + '.pin')).write_text(self.blob(index))
