#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Check failed named COPY releases both readers and destination mounts."""
import argparse
import json
from pathlib import Path


def verify(run):
    serial = (run / 'serial.txt').read_text()
    checks = {
        'source_completed': '\nNAMED_COPY_SOURCE_READY\n' in serial,
        'copy_failed': 'directive 0' in serial and 'No such file or directory' in serial,
        'no_continuation': '\nNAMED_COPY_UNEXPECTED_CONTINUATION\n' not in serial,
        'build_rejected': (run / 'status.raw').read_bytes().rstrip(b'\0') == b'DISTRO_BUILD_EXIT 1\n',
        'output_absent': '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial,
        'cleanup_completed': not any(message in serial for message in (
            'source reader cleanup:', 'COPY cleanup:', 'runtime cleanup:',
            'runtime retained', 'retained until VM disposal')),
    }
    result = {'checks': checks, 'passed': all(checks.values())}
    (run / 'copy-failure-assessment.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    args = parser.parse_args()
    result = verify(args.run)
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
