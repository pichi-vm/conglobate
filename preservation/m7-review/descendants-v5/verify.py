#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess the deterministic background-child fixture after CLI export."""
import argparse
import json
from pathlib import Path


class DescendantEvidence:
    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        checks = {marker: f'\n{marker}\n' in serial for marker in (
            'BACKGROUND_CHILD_STARTED_PASS', 'BACKGROUND_CHILD_DRAINED_PASS',
            'DISTRO_BUILD_AND_EXPORT_PASS')}
        checks['exit_zero'] = (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 0\n'
        checks['artifact_exported'] = (self.run / 'artifact/index.json').is_file()
        report = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'descendant-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = DescendantEvidence(args.run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
