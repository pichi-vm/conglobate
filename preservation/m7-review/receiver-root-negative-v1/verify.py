#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Differential check: tool labeling works, a second filesystem probe is denied."""
import argparse
import json
from pathlib import Path
import re


class ProbeEvidence:
    def __init__(self, run, negative):
        self.run = run
        self.negative = negative

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        observed = re.findall(r'^LABEL_FAULT_OBSERVATION (\d+) (\d+) (\d+)$', serial, re.M)
        if len(observed) != 1:
            raise ValueError('exactly one trusted fault observation required')
        device, allowed, denied = map(int, observed[0])
        status = (self.run / 'status.raw').read_bytes().rstrip(b'\0')
        checks = {'first_filesystem_observed': device != 0,
                  'first_probe_allowed': allowed == 1,
                  'denial_count': denied == int(self.negative),
                  'runtime_cleanup_complete': 'runtime cleanup:' not in serial and 'runtime retained' not in serial}
        if self.negative:
            checks.update(
                build_rejected=status == b'DISTRO_BUILD_EXIT 1\n',
                command_not_admitted='BOOTSTRAP_TWO_ROOTS_FIRST_STEP_PASS' not in serial,
                label_denial='restricted labeling validation: environment labeling support: Operation not permitted' in serial,
                cleanup_complete='cleanup: complete' in serial,
                output_absent='\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial)
        else:
            checks.update(build_succeeded=status == b'DISTRO_BUILD_EXIT 0\n',
                          command_admitted='\nCOMMAND_LABEL_ADMISSION_PASS\n' in serial,
                          exported='\nDISTRO_BUILD_AND_EXPORT_PASS\n' in serial)
        report = {'negative': self.negative, 'observed': {'first_device': device, 'allowed': allowed, 'denied': denied},
                  'checks': checks, 'passed': all(checks.values()), 'full_sandbox_qualified': False}
        (self.run / 'probe-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('--negative', action='store_true')
    args = parser.parse_args()
    result = ProbeEvidence(args.run, args.negative).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
