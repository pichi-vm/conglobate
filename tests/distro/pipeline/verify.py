#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify the offline full-pipeline fixture, including stage-level layer count."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import cbor2


class PipelineEvidence:
    def __init__(self, run, imported_boot=False):
        self.run = run
        self.imported_boot = imported_boot
        self.artifact = run / 'artifact'

    def blob(self, descriptor):
        algorithm, digest = descriptor['digest'].split(':')
        assert algorithm == 'sha256'
        path = self.artifact / 'blobs' / algorithm / digest
        data = path.read_bytes()
        assert len(data) == descriptor['size']
        assert hashlib.sha256(data).hexdigest() == digest
        return path, data

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        markers = ['PIPELINE_BOOTSTRAP_PASS', 'PIPELINE_CARAPACE_FIRST_PASS',
                       'PIPELINE_CARAPACE_LAST_PASS',
                       'MULTISTAGE_ARTIFACT_PRODUCER_PASS',
                       'DISTRO_BUILD_AND_EXPORT_PASS']
        if not self.imported_boot:
            markers.append('PIPELINE_FIRMWARE_TOOLS_PASS')
        for marker in markers:
            assert f'\n{marker}\n' in serial, marker
        assert (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        raw = (self.artifact / 'inventory.cbor').read_bytes()
        inventory = cbor2.loads(raw)
        assert cbor2.dumps(inventory, canonical=True) == raw
        assert inventory['version'] == 3
        assert len(inventory['inputs']) == 1
        assert inventory['inputs'][0]['roles'] == ['bootstrap-tools',
            'carapace-base' if self.imported_boot else 'firmware-tools']
        execution = inventory['execution']
        assert execution['id_mappings'] == {
            'uid': {'parent_start': 200000, 'count': 131072},
            'gid': {'parent_start': 400000, 'count': 131072}}

        assert execution['network_steps'] == []
        assert len(execution['formatters']) == 1
        assert execution['formatters'][0]['stage'] == 'bootstrap'
        assert execution['transform']['target_present'] is True
        assert execution['transform']['default_skipped'] is False
        output = inventory['outputs']
        # One bootstrap layer plus ONE carapace layer despite three directives.
        assert len(output['scutes']) == 2
        assert all(scute['origin'] == 'produced' for scute in output['scutes'])
        index = json.loads((self.artifact / 'index.json').read_text())
        assert len(index['manifests']) == 1
        descriptor = index['manifests'][0]
        assert descriptor['digest'] == 'sha256:' + output['manifest']['digest'].hex()
        _, manifest_bytes = self.blob(descriptor)
        manifest = json.loads(manifest_bytes)
        self.blob(manifest['config'])
        layers = manifest['layers']
        assert len(layers) == 4
        assert sorted(layer['mediaType'] for layer in layers) == [
            'application/vnd.pichi.dtb.v1', 'application/vnd.pichi.pmi.v1',
            'application/vnd.pichi.scute.v1', 'application/vnd.pichi.scute.v1']
        scutes = [layer for layer in layers if layer['mediaType'] == 'application/vnd.pichi.scute.v1']
        assert [layer['digest'] for layer in scutes] == [
            'sha256:' + scute['digest']['digest'].hex() for scute in output['scutes']]
        for layer in layers:
            path, data = self.blob(layer)
            if layer['mediaType'] == 'application/vnd.pichi.pmi.v1':
                assert data == (self.run / 'root/context/firmware/boot.pmi').read_bytes()
                assert layer['digest'] == 'sha256:' + output['pmi']['digest'].hex()
            if layer['mediaType'] == 'application/vnd.pichi.dtb.v1':
                cmdline = subprocess.check_output(
                    ['fdtget', '-t', 's', str(path), '/chosen', 'bootargs'], text=True)
                assert 'corium=' + output['carapace_root'].hex() in cmdline.split()
                assert layer['digest'] == 'sha256:' + output['dtb']['digest'].hex()
        expected = {item['digest'].split(':')[1]
                    for item in [descriptor, manifest['config'], *layers]}
        assert {path.name for path in (self.artifact / 'blobs/sha256').iterdir()} == expected
        return {'full_pipeline_passed': True, 'finalized_artifact_copy': True,
                'orphan_excluded_without_lock_or_network_input': True, 'one_carapace_scute': True,
                'bootstrap_and_firmware_tools_not_inherited': True,
                'root_bound_in_dtb': True, 'inventory_and_blob_graph_verified': True,
                'boot_qualified': False, 'full_sandbox_qualified': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('--imported-boot', action='store_true')
    arguments = parser.parse_args()
    result = PipelineEvidence(arguments.run, arguments.imported_boot).verify()
    (arguments.run / 'pipeline-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
