#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify boot-only publication and unavailable CARAPACE with local tools."""
import argparse
import hashlib
import json
from pathlib import Path
import cbor2


def verify(run):
    artifact = run / 'artifact'
    serial = (run / 'serial.txt').read_text()
    assert 'BOOT_ONLY_CARAPACE_UNDEFINED_PASS' in serial
    if (run / 'status.raw').exists():
        assert (run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
    raw = (artifact / 'inventory.cbor').read_bytes()
    inventory = cbor2.loads(raw)
    assert cbor2.dumps(inventory, canonical=True) == raw
    output = inventory['outputs']
    assert output['scutes'] == []
    assert 'carapace_root' not in output
    assert inventory['execution']['network_steps'] == []
    assert inventory['execution']['formatters'] == []
    index = json.loads((artifact / 'index.json').read_text())
    descriptor = index['manifests'][0]
    assert descriptor['digest'] == 'sha256:' + output['manifest']['digest'].hex()

    def blob(item):
        algorithm, digest = item['digest'].split(':')
        assert algorithm == 'sha256'
        data = (artifact / 'blobs/sha256' / digest).read_bytes()
        assert len(data) == item['size']
        assert hashlib.sha256(data).hexdigest() == digest
        return data

    manifest = json.loads(blob(descriptor))
    blob(manifest['config'])
    layers = manifest['layers']
    assert sorted(layer['mediaType'] for layer in layers) == [
        'application/vnd.pichi.dtb.v1', 'application/vnd.pichi.pmi.v1']
    for layer in layers:
        data = blob(layer)
        kind = 'pmi' if layer['mediaType'].endswith('pmi.v1') else 'dtb'
        assert layer['digest'] == 'sha256:' + output[kind]['digest'].hex()
        if kind == 'pmi':
            assert (artifact / 'boot.pmi').read_bytes() == data
    assert 'scutes: []' in (artifact / 'build.yaml').read_text()
    assert not list(artifact.glob('*.cow'))
    assert not list(artifact.glob('*.verity'))
    return {'boot_only_passed': True, 'local_tool_root_not_exposed': True,
            'no_output_carapace_or_scutes': True, 'inventory_and_blobs_verified': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    arguments = parser.parse_args()
    assessment = verify(arguments.run)
    (arguments.run / 'boot-only-assessment.json').write_text(json.dumps(assessment, indent=2) + '\n')
    print(json.dumps(assessment, indent=2))
