#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess plan-command xattr behavior on both bootstrap mounts."""
import argparse
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from artifact import build_manifest


class LabelOperationsEvidence:
    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        checks = {marker: f'\n{marker}\n' in serial for marker in (
            'COMMAND_TWO_ROOT_LABEL_WRITE_PASS', 'COMMAND_TWO_ROOT_LABEL_REMOUNT_PASS',
            'DISTRO_BUILD_AND_EXPORT_PASS')}
        checks['exit_zero'] = (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        manifest = build_manifest(self.run)
        checks['build_annotations_verified'] = bool(manifest)
        checks['new_output_only'] = [layer['mediaType'] for layer in manifest['layers']] == [
            'application/vnd.pichi.scute.v1']
        report = {'checks': checks, 'passed': all(checks.values()),
                  'differential_admission_probe_qualified': False, 'full_sandbox_qualified': False}
        (self.run / 'label-operations-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = LabelOperationsEvidence(args.run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
