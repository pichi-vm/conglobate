#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Verify a derived root retains PMI bytes and rebinds inherited bootconfig."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from artifact import build_manifest


def blob(directory, descriptor):
    algorithm, digest = descriptor['digest'].split(':')
    assert algorithm == 'sha256'
    data = (directory / 'blobs/sha256' / digest).read_bytes()
    assert len(data) == descriptor['size']
    assert hashlib.sha256(data).hexdigest() == digest
    return data


def verify(run, cache, pin):
    index_bytes = (cache / 'blobs/sha256' / pin.removeprefix('sha256:')).read_bytes()
    assert 'sha256:' + hashlib.sha256(index_bytes).hexdigest() == pin
    index = json.loads(index_bytes)
    source = json.loads(blob(cache, index['manifests'][0]))
    output = build_manifest(run)
    artifact = run / 'artifact'
    assert '\nINHERITED_BOOT_EXTENSION_PASS\n' in (run / 'serial.txt').read_text()
    assert (run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
    for layer in output['layers']:
        blob(artifact, layer)
    scutes = lambda manifest: [layer for layer in manifest['layers']
                               if layer['mediaType'].startswith('application/vnd.pichi.scute.v1')]
    assert scutes(output)[:-1] == scutes(source)
    assert len(scutes(output)) == len(scutes(source)) + 1
    pmi = lambda manifest: next(layer for layer in manifest['layers']
                                if layer['mediaType'] == 'application/vnd.pichi.pmi.v1')
    assert pmi(output) == pmi(source)
    assert blob(artifact, pmi(output)) == blob(cache, pmi(source))
    root = output['annotations']['dev.pichi.carapace.verity.hash']
    assert root != source['annotations']['dev.pichi.carapace.verity.hash']
    config = next(layer for layer in output['layers']
                  if layer['mediaType'] == 'application/vnd.pichi.bootconfig.v1')
    # This fixture's seed config contains only corium.root.
    assert blob(artifact, config).decode() == f'corium.root = "{root}";\n'
    assert output['config'] == source['config']
    for layer in source['layers']:
        blob(cache, layer)
    result = {'passed': True, 'one_new_layer': True, 'pmi_unchanged': True,
              'inherited_root_rebound': True, 'parent_blobs_unchanged': True,
              'boot_qualified': False}
    (run / 'extension-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--pin', required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.run, args.cache, args.pin), indent=2))
