#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check that typed namespace entry rejects a non-network namespace before admission."""
import argparse
import json
from pathlib import Path


class WrongTypeEvidence:
    def __init__(self, run):
        self.run = run

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        checks = {
            'build_rejected': (self.run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 1\n',
            'command_not_admitted': 'WRONG_NETWORK_TYPE_COMMAND_ADMITTED' not in serial,
            'namespace_entry_rejected': 'Invalid argument' in serial or 'EINVAL' in serial,
            'output_absent': 'FAILED_BUILD_OUTPUT_ABSENT' in serial,
        }
        result = {'checks': checks, 'passed': all(checks.values()),
                  'full_sandbox_qualified': False}
        (self.run / 'wrong-type-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    result = WrongTypeEvidence(parser.parse_args().run).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
