#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check the minimal bootstrap regression's exported graph and inventory."""
import argparse
import hashlib
import json
from pathlib import Path
import cbor2


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
        raw = (self.artifact / 'inventory.cbor').read_bytes()
        inventory = cbor2.loads(raw)
        assert cbor2.dumps(inventory, canonical=True) == raw
        assert inventory['version'] == 3
        assert len(inventory['inputs']) == 1
        assert inventory['inputs'][0]['roles'] == ['bootstrap-tools']
        output = inventory['outputs']
        assert 'pmi' not in output and 'dtb' not in output
        assert len(output['scutes']) == 1
        assert output['scutes'][0]['origin'] == 'produced'
        execution = inventory['execution']
        assert execution['network_steps'] == []
        assert execution['transform']['default_skipped'] is True
        assert execution['transform']['target_present'] is False
        assert len(execution['formatters']) == 1
        formatter = execution['formatters'][0]
        assert formatter['stage'] == 'bootstrap'
        assert formatter['filesystem'] == {
            'type': 'ext4', 'label': 'label-probe', 'size_bytes': 64 * 1024**2,
            'logical_size_bytes': 64 * 1024**3,
        }
        inputs = json.loads((self.run / 'inputs.json').read_text())
        for entry in formatter['runtime']:
            # The profile generates its own configuration, separately measured.
            if entry['path'] != 'etc/mke2fs.conf':
                runtime_root = (self.run / 'root').resolve()
                path = (runtime_root / entry['path']).resolve().relative_to(runtime_root)
                assert entry['digest']['digest'].hex() == inputs[str(path)]
        assert formatter['arguments'][-2:] == ['/dev/output', '16384']
        index = json.loads((self.artifact / 'index.json').read_text())
        assert len(index['manifests']) == 1
        descriptor = index['manifests'][0]
        assert descriptor['digest'] == 'sha256:' + output['manifest']['digest'].hex()
        manifest = json.loads(self.descriptor(descriptor))
        self.descriptor(manifest['config'])
        assert len(manifest['layers']) == 1
        layer = manifest['layers'][0]
        assert layer['mediaType'] == 'application/vnd.pichi.scute.v1'
        assert layer['digest'] == 'sha256:' + output['scutes'][0]['digest']['digest'].hex()
        self.descriptor(layer)
        expected = {item['digest'].split(':')[1]
                    for item in (descriptor, manifest['config'], layer)}
        assert {p.name for p in (self.artifact / 'blobs/sha256').iterdir()} == expected
        return {'two_root_steps_passed': True, 'export_graph_verified': True,
                'formatter_receipt_verified': True, 'tool_components_not_inherited': True,
                'inventory_canonical': True, 'bootable_distro_qualified': False,
                'full_sandbox_qualified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = BootstrapEvidence(args.run).verify()
    (args.run / 'bootstrap-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
