"""Committed pass-through CLI inputs; no host privileged operations."""
import pathlib
import subprocess
import json
import struct
from cache_fixture import CacheFixture


class PipelineFixture:
    def __init__(self, root):
        self.root = root

    @classmethod
    def cbor(cls, value):
        # Independent fixture encoder: text, nonnegative integers, maps and arrays.
        if isinstance(value, str):
            data = value.encode()
            return cls.head(3, len(data)) + data
        if isinstance(value, int):
            return cls.head(0, value)
        if isinstance(value, list):
            return cls.head(4, len(value)) + b''.join(cls.cbor(v) for v in value)
        return cls.head(5, len(value)) + b''.join(cls.cbor(k) + cls.cbor(v) for k, v in value.items())

    @staticmethod
    def head(kind, count):
        if count < 24:
            return bytes([kind * 32 + count])
        if count < 256:
            return bytes([kind * 32 + 24, count])
        return bytes([kind * 32 + 25]) + struct.pack('>H', count)

    @classmethod
    def pmi(cls):
        spec = {'version': 1, 'vm:vcpu': {}, 'cpu:profile': 'x86-64-v2',
                'actions': [{'type': 'fill', 'gpa': 8192, 'section': '.dtb', 'kind': 'dt:dtb'}]}
        manifest = cls.cbor(spec)
        data = bytearray(1024)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 0x3c, 0x80)
        data[0x80:0x84] = b'PE\0\0'
        for offset, value in [(0x84, 0x8664), (0x86, 2), (0x94, 240), (0x96, 0x22), (0x98, 0x20b), (0x98+68, 10)]:
            struct.pack_into('<H', data, offset, value)
        for offset, value in [(0x98+32, 4096), (0x98+36, 512), (0x98+56, 0x20000), (0x98+60, 512)]:
            struct.pack_into('<I', data, offset, value)
        section = 0x98+240
        data[section:section+7] = b'.pmi.vm'
        for offset, value in [(8, len(manifest)), (12, 4096), (16, 512), (20, 512), (36, 0x42000040)]:
            struct.pack_into('<I', data, section+offset, value)
        section += 40
        data[section:section+4] = b'.dtb'
        for offset, value in [(8, 4096), (12, 8192), (36, 0xc0000080)]:
            struct.pack_into('<I', data, section+offset, value)
        data[512:512+len(manifest)] = manifest
        return bytes(data)

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
        for kind, data in [('pmi', self.pmi()), ('dtb', dtb)]:
            manifest['layers'].append({'mediaType': f'application/vnd.pichi.{kind}.v1',
                                      'digest': cache.blob(data), 'size': len(data)})
        manifest['x-preserved'] = {'value': 42}
        data = json.dumps(manifest).encode()
        index['manifests'][0].update(digest=cache.blob(data), size=len(data))
        firmware_pin = cache.blob(json.dumps(index).encode())
        with (repo / 'images.lock').open('a') as lock:
            lock.write(f'  example.test/firmware:1:\n    index: {firmware_pin}\n')
        opaque = b'opaque fixture firmware'
        for layer in manifest['layers']:
            if layer['mediaType'] == 'application/vnd.pichi.pmi.v1':
                layer.update(digest=cache.blob(opaque), size=len(opaque))
        data = json.dumps(manifest).encode()
        index['manifests'][0].update(digest=cache.blob(data), size=len(data))
        opaque_pin = cache.blob(json.dumps(index).encode())
        with (repo / 'images.lock').open('a') as lock:
            lock.write(f'  example.test/opaque:1:\n    index: {opaque_pin}\n')
        base = 'carapace:\n  from: example.test/firmware:1\n'
        cases = {
            'default': base,
            'opaque-changed': 'carapace:\n  from: example.test/opaque:1\n',
            'opaque-unchanged': 'carapace:\n  from: example.test/opaque:1\ntransform: []\n',
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
