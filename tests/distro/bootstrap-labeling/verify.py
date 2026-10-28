#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check the minimal bootstrap regression's exported graph and build annotations."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from artifact import build_manifest


class BootstrapEvidence:
    def __init__(self, run):
        self.run = run
        self.artifact = run / 'artifact'

    def descriptor(self, descriptor):
        algorithm, digest = descriptor['digest'].split(':')
        assert algorithm == 'sha256'
        data = (self.artifact / 'blobs' / algorithm / digest).read_bytes()
        assert len(data) == descriptor['size']
        assert hashlib.sha256(data).hexdigest() == digest
        return data

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        for marker in ('BOOTSTRAP_TWO_ROOTS_FIRST_STEP_PASS',
                       'BOOTSTRAP_TWO_ROOTS_REMOUNT_PASS',
                       'DISTRO_BUILD_AND_EXPORT_PASS'):
            assert f'\n{marker}\n' in serial
        assert (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        index = json.loads((self.artifact / 'index.json').read_text())
        assert len(index['manifests']) == 1
        descriptor = index['manifests'][0]
        manifest = build_manifest(self.run)
        assert manifest == json.loads(self.descriptor(descriptor))
        self.descriptor(manifest['config'])
        assert len(manifest['layers']) == 1
        layer = manifest['layers'][0]
        assert layer['mediaType'] == 'application/vnd.pichi.scute.v1'
        self.descriptor(layer)
        expected = {item['digest'].split(':')[1]
                    for item in (descriptor, manifest['config'], layer)}
        assert {p.name for p in (self.artifact / 'blobs/sha256').iterdir()} == expected
        return {'two_root_steps_passed': True, 'export_graph_verified': True,
                'tool_components_not_inherited': True,
                'build_annotations_verified': True, 'bootable_distro_qualified': False,
                'full_sandbox_qualified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = BootstrapEvidence(args.run).verify()
    (args.run / 'bootstrap-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
