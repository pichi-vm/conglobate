# SPDX-License-Identifier: Apache-2.0
"""Independent checks of the published manifest's committed build binding."""
import hashlib
import json
import subprocess


def build_manifest(run):
    artifact = run / 'artifact'
    assert {entry.name for entry in artifact.iterdir()} == {'oci-layout', 'index.json', 'blobs'}
    index = json.loads((artifact / 'index.json').read_text())
    assert len(index['manifests']) == 1
    descriptor = index['manifests'][0]
    algorithm, digest = descriptor['digest'].split(':')
    raw = (artifact / 'blobs' / algorithm / digest).read_bytes()
    assert algorithm == 'sha256'
    assert hashlib.sha256(raw).hexdigest() == digest
    assert len(raw) == descriptor['size']
    manifest = json.loads(raw)
    commit = subprocess.check_output(
        ['git', '-C', str(run / 'root/context'), 'rev-parse', 'HEAD'], text=True).strip()
    expected = {'dev.pichi.build.commit': ('sha1:' if len(commit) == 40 else 'sha256:') + commit,
                'dev.pichi.build.context': '.', 'dev.pichi.build.plan': 'plan.yaml',
                'dev.pichi.build.lock': 'images.lock'}
    annotations = manifest['annotations']
    assert {key: value for key, value in annotations.items()
            if key.startswith('dev.pichi.build.')} == expected
    parents = {}
    for layer in manifest['layers']:
        if not layer['mediaType'].startswith('application/vnd.pichi.scute.v1'):
            continue
        algorithm, digest = layer['digest'].split(':')
        data = (artifact / 'blobs' / algorithm / digest).read_bytes()
        assert algorithm == 'sha256' and hashlib.sha256(data).hexdigest() == digest
        assert len(data) == layer['size']
        if layer['mediaType'].endswith('+zstd'):
            data = subprocess.check_output(['zstd', '-q', '-d', '-c'], input=data)
        blocks = int(layer['annotations']['dev.pichi.scute.verity.data-blocks'])
        assert blocks > 0 and (len(data) + 4095) // 4096 == blocks
        salt = bytes.fromhex(layer['annotations']['dev.pichi.scute.verity.salt'])
        assert len(salt) >= 32
        root = scute_root(data.ljust(blocks * 4096, b'\0'), salt)
        assert root not in parents
        parents[root] = salt[:32]
    if parents:
        root = bytes.fromhex(annotations['dev.pichi.carapace.verity.hash'])
        assert len(root) == 32 and root != bytes(32)
        while root != bytes(32):
            root = parents.pop(root)
        assert not parents
    else:
        assert 'dev.pichi.carapace.verity.hash' not in annotations
    return manifest


def scute_root(data, salt):
    """Derive dm-verity v1 sha256 root from decoded, block-padded scute data."""
    assert data and len(data) % 4096 == 0
    while len(data) > 4096:
        hashes = b''.join(hashlib.sha256(salt + data[i:i + 4096]).digest()
                          for i in range(0, len(data), 4096))
        data = hashes.ljust((len(hashes) + 4095) // 4096 * 4096, b'\0')
    return hashlib.sha256(salt + data).digest()
