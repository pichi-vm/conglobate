#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check stage-relative failure position for separately sealed carapace steps."""
import argparse
import json
from pathlib import Path


class DirectiveIndexEvidence:
    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        status = (self.run / 'status.raw').read_bytes().rstrip(b'\0')
        checks = {
            'build_rejected': status == b'DISTRO_BUILD_EXIT 1\n',
            'first_directive_completed': 'FIRST_DIRECTIVE_COMPLETE' in serial,
            'second_directive_reported': 'directive 1: command outcome:' in serial,
            'no_incorrect_first_position': 'directive 0:' not in serial,
            'cleanup_completed': 'cleanup: complete' in serial,
            'no_exported_artifact': not (self.run / 'artifact').exists(),
        }
        report = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'directive-index-assessment.json').write_text(
            json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    result = DirectiveIndexEvidence(parser.parse_args().run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
