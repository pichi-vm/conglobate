#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Assess expected CLI failure without mistaking build failure for cleanup success."""
import argparse
import json
from pathlib import Path


class FailureEvidence:
    def __init__(self, run, mode):
        self.run = run
        self.mode = mode

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        marker = {'nonzero': 'COMMAND_EXPECTED_FAILURE',
                  'timeout': 'COMMAND_TIMEOUT_STARTED'}[self.mode]
        diagnostic = {'nonzero': 'command exited with exit status: 7',
                      'timeout': 'deadline expired'}[self.mode]
        checks = {
            'command_started': f'\n{marker}\n' in serial,
            'expected_error': diagnostic in serial,
            'output_not_published': '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial,
            'build_rejected': (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 1\n',
            'command_cleanup_complete': 'cleanup: complete' in serial,
            'runtime_cleanup_complete': 'runtime cleanup:' not in serial and 'runtime retained' not in serial,
        }
        report = {'mode': self.mode, 'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'failure-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('mode', choices=['nonzero', 'timeout'])
    arguments = parser.parse_args()
    result = FailureEvidence(arguments.run, arguments.mode).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
