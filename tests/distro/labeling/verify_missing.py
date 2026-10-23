#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess missing-policy admission failure from the preserved trusted VM result."""
import argparse
import json
from pathlib import Path


class MissingPolicyEvidence:
    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        status = self.run / 'status.raw'
        checks = {
            'build_rejected': status.is_file() and status.read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 1\n',
            'command_not_admitted': 'COMMAND_LABEL_ADMISSION_PASS' not in serial,
            'label_denial_diagnostic': 'restricted labeling validation: environment labeling support: Operation not permitted' in serial,
            'command_cleanup_complete': 'cleanup: complete' in serial,
            'runtime_cleanup_complete': 'runtime cleanup:' not in serial and 'runtime retained' not in serial,
            'output_absent': '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial,
        }
        report = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'label-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = MissingPolicyEvidence(args.run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
