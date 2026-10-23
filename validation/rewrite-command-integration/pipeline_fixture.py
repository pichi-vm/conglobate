"""Committed pass-through CLI inputs; no host privileged operations."""
import pathlib
import subprocess
import json
import struct
from cache_fixture import CacheFixture


class PipelineFixture:
    def __init__(self, root):
        self.root = root

    def create(self):
        repo = self.root / 'pipeline-context'
        repo.mkdir()
        (repo / 'recipes').mkdir()
        recipe = 'carapace:\n  from: example.test/base:1\n'
        (repo / 'recipes/image.yml').write_text(recipe)
        (repo / 'recipes/unsupported.yml').write_text(recipe + '  derive:\n    - run: echo MUST_NOT_RUN\n')
        pin = (self.root / 'cache-fixture/chain.pin').read_text().strip()
        (repo / 'images.lock').write_text(f'version: 1\nimages:\n  example.test/base:1:\n    index: {pin}\n')
        cache = CacheFixture(self.root / 'cache-fixture')
        index = json.loads((cache.root / 'blobs/sha256' / pin.split(':')[1]).read_bytes())
        manifest = json.loads((cache.root / 'blobs/sha256' / index['manifests'][0]['digest'].split(':')[1]).read_bytes())
        # A small independent FDT encoder for a single bootargs property.
        value = b'quiet corium=old\0'
        structure = struct.pack('>I', 1) + bytes(4)
        structure += struct.pack('>I', 1) + b'chosen\0\0'
        structure += struct.pack('>III', 3, len(value), 0) + value
        structure += bytes((-len(structure)) % 4)
        structure += struct.pack('>III', 2, 2, 9)
        strings = b'bootargs\0'
        dtb = struct.pack('>10I', 0xd00dfeed, 56 + len(structure) + len(strings),
                          56, 56 + len(structure), 40, 17, 16, 7, len(strings), len(structure))
        dtb += bytes(16) + structure + strings
        for kind, data in [('pmi', b'opaque fixture firmware'), ('dtb', dtb)]:
            manifest['layers'].append({'mediaType': f'application/vnd.pichi.{kind}.v1',
                                      'digest': cache.blob(data), 'size': len(data)})
        manifest['x-preserved'] = {'value': 42}
        data = json.dumps(manifest).encode()
        index['manifests'][0].update(digest=cache.blob(data), size=len(data))
        firmware_pin = cache.blob(json.dumps(index).encode())
        with (repo / 'images.lock').open('a') as lock:
            lock.write(f'  example.test/firmware:1:\n    index: {firmware_pin}\n')
        base = 'carapace:\n  from: example.test/firmware:1\n'
        cases = {
            'default': base,
            'authored': base + "transform:\n  - {op: set, property: /chosen/bootargs, type: string, data: 'authored ${CARAPACE}'}\n",
            'missing-property': base + 'transform:\n  - {op: remove, property: /missing/value}\n',
            'missing-dtb': recipe + 'transform: []\n',
        }
        for name, content in cases.items():
            (repo / f'recipes/{name}.yml').write_text(content)
        subprocess.run(['git', 'init' , '--quiet', str(repo)], check=True)
        subprocess.run(['git', '-C', str(repo), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(repo), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                        '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'Pipeline fixture'], check=True)
        (self.root / 'pipeline-commit').write_text(subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True))
        (self.root / 'pipeline-recipe-committed').write_text(recipe)
        # The production binary must read the requested commit, not this worktree.
        (repo / 'recipes/image.yml').write_text('not the committed recipe\n')
