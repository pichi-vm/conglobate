#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Combine recipe/service observations with independent build status and cleanup."""
import argparse
import json
from pathlib import Path


class NetworkEvidence:
    def __init__(self, run, mode):
        self.run, self.mode = run, mode

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        status = (self.run / 'status.raw').read_bytes().rstrip(b'\0')
        checks = {marker: f'\n{marker}\n' in serial for marker in (
            'NETWORK_RESTRICTED_IMAGE_DNS_AND_TCP_PASS',
            'NETWORK_RECIPE_DNS_TCP_AND_READONLY_RESOLVER_PASS',
            'NETWORK_BUILD_RESOURCE_CLEANUP_PASS')}
        checks['runtime_cleanup'] = 'runtime cleanup:' not in serial and 'runtime retained' not in serial
        if self.mode == 'success':
            checks['following_offline_step'] = '\nNETWORK_FOLLOWING_OFFLINE_STEP_PASS\n' in serial
            checks['build_export'] = '\nDISTRO_BUILD_AND_EXPORT_PASS\n' in serial
            checks['independent_status'] = status == b'DISTRO_BUILD_EXIT 0\n'
        else:
            checks['build_rejected'] = status == b'DISTRO_BUILD_EXIT 1\n'
            checks['output_absent'] = '\nFAILED_BUILD_OUTPUT_ABSENT\n' in serial
            checks['command_cleanup'] = 'cleanup: complete' in serial
            marker = 'COMMAND_EXPECTED_FAILURE' if self.mode == 'nonzero' else 'COMMAND_TIMEOUT_STARTED'
            checks['failure_reached'] = f'\n{marker}\n' in serial
            diagnostic = 'command exited with exit status: 7' if self.mode == 'nonzero' else 'deadline expired'
            checks['expected_error'] = diagnostic in serial
        report = {'mode': self.mode, 'checks': checks, 'passed': all(checks.values()),
                  'external_outbound_nat_qualified': False, 'full_sandbox_qualified': False}
        (self.run / 'network-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('mode', choices=['success', 'nonzero', 'timeout'])
    args = parser.parse_args()
    result = NetworkEvidence(args.run, args.mode).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
