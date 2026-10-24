#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Rootless OCI interoperability regression, migrated from the VM prototype."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'lifecycle'))
from cache_fixture import CacheFixture


class Interoperability:
    def __init__(self, arguments):
        self.arguments = arguments
        self.root = arguments.output.resolve()

    def run(self):
        self.root.mkdir()
        cache = self.root / 'cache'
        # Synthetic valid snapshot/verity data: no filesystem mount or VM needed.
        CacheFixture(cache).chains()
        pin = (cache / 'chain.pin').read_text().strip()
        context = self.root / 'context'
        context.mkdir()
        (context / 'recipe.yaml').write_text('carapace:\n  from: qualification.local/fixture:ci\n')
        (context / 'images.lock').write_text(
            f'version: 1\nimages:\n  qualification.local/fixture:ci:\n    index: {pin}\n')
        for command in (['init', '-q'], ['add', '.'],
                        ['-c', 'user.name=OCI test', '-c', 'user.email=test@invalid',
                         'commit', '-qm', 'Pin OCI fixture']):
            subprocess.run(['git', '-C', str(context), *command], check=True)
        commit = subprocess.check_output(['git', '-C', str(context), 'rev-parse', 'HEAD'], text=True).strip()
        artifact = self.root / 'artifact'
        # Pass-through performs no privileged operations; map only our own UID.
        subprocess.run(['unshare', '--user', '--map-root-user', '--',
                        str(self.arguments.binary.resolve()), 'build', '--repository', str(context),
                        '--commit', commit, '--recipe', 'recipe.yaml', '--lock', 'images.lock',
                        '--cache', str(cache), '--output', str(artifact)], check=True)
        digest = json.loads((artifact / 'index.json').read_text())['manifests'][0]['digest']
        copied = self.root / 'oras-copy'
        subprocess.run([str(self.arguments.oras.resolve()), 'cp', '--from-oci-layout',
                        '--to-oci-layout', f'{artifact}@{digest}', f'{copied}:verified'], check=True)
        assert json.loads((copied / 'index.json').read_text())['manifests'][0]['digest'] == digest
        original = artifact / 'blobs/sha256'
        saved = copied / 'blobs/sha256'
        assert {p.name for p in original.iterdir()} == {p.name for p in saved.iterdir()}
        for path in original.iterdir():
            data = path.read_bytes()
            assert hashlib.sha256(data).hexdigest() == path.name
            assert (saved / path.name).read_bytes() == data
        assert (artifact / 'inventory.cbor').is_file()
        (self.root / 'assessment.json').write_text(json.dumps({
            'oras_graph_copy_passed': True, 'manifest': digest,
            'blobs': len(list(original.iterdir())), 'privileged_operations': False,
        }, indent=2) + '\n')
        print('OCI_ORAS_SELF_CONTAINED_COPY_PASS')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--oras', type=Path, required=True)
    Interoperability(parser.parse_args()).run()
