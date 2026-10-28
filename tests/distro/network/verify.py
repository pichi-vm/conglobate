#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Combine plan/service observations with independent build status and cleanup."""
import argparse
import json
from pathlib import Path


class NetworkEvidence:
    def __init__(self, run, mode):
        self.run, self.mode = run, mode

    def verify(self):
        serial = (self.run / 'serial.txt').read_text()
        status = (self.run / 'status.raw').read_bytes().rstrip(b'\0')
        markers = ('NETWORK_EXTERNAL_TCP_REPLY_PASS', 'NETWORK_ENVIRONMENT_RETAINED_PASS') if self.mode in ('external', 'external-nat') else (
            'NETWORK_RESTRICTED_IMAGE_DNS_AND_TCP_PASS',
            'NETWORK_PLAN_DNS_TCP_AND_READONLY_RESOLVER_PASS',
            'NETWORK_ENVIRONMENT_RETAINED_PASS')
        checks = {marker: f'\n{marker}\n' in serial for marker in markers}
        if self.mode in ('external', 'external-nat'):
            checks['independent_external_exchange'] = json.loads(
                (self.run / 'external-service.json').read_text())['passed']
        if self.mode == 'external-nat':
            checks['source_translation'] = '\nNETWORK_EXTERNAL_IPV4_SOURCE_NAT_PASS\n' in serial
        checks['runtime_cleanup'] = 'runtime cleanup:' not in serial and 'runtime retained' not in serial
        if self.mode in ('success', 'external', 'external-nat'):
            if self.mode == 'success':
                checks['shared_enabled_namespace'] = '\nNETWORK_ENABLED_STEPS_REUSE_NAMESPACE_PASS\n' in serial
                checks['restricted_command_capabilities'] = '\nNETWORK_COMMAND_CAPABILITIES_RESTRICTED_PASS\n' in serial
                checks['resolver_restored'] = '\nNETWORK_RESOLVER_SYMLINK_AND_CONTENT_RESTORED_PASS\n' in serial
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
                  'external_ipv4_tcp_observed': self.mode in ('external', 'external-nat') and all(checks.values()),
                  'ipv4_source_translation_observed': self.mode == 'external-nat' and all(checks.values()),
                  'external_outbound_nat_qualified': False, 'full_sandbox_qualified': False}
        (self.run / 'network-assessment.json').write_text(json.dumps(report, indent=2) + '\n')
        return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', type=Path)
    parser.add_argument('mode', choices=['success', 'nonzero', 'timeout', 'external', 'external-nat'])
    args = parser.parse_args()
    result = NetworkEvidence(args.run, args.mode).verify()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
