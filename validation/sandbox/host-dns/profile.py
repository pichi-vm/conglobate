#!/usr/bin/env python3
"""Read-only host resolver summary; omit addresses, names and domain strings."""
import argparse
import datetime
import ipaddress
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()


def read(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=10)
    return result.returncode, result.stdout


report = {'observed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'scope': 'configuration observation, not DNS reachability or VPN routing proof'}
resolver = Path('/etc/resolv.conf')
report['resolver_is_symlink'] = resolver.is_symlink()
report['resolver_is_systemd_stub'] = resolver.resolve() == Path('/run/systemd/resolve/stub-resolv.conf')
servers = []
for line in resolver.read_text().splitlines():
    parts = line.split()
    if len(parts) > 1 and parts[0] == 'nameserver':
        address = ipaddress.ip_address(parts[1])
        servers.append({'family': address.version, 'loopback': address.is_loopback})
report['resolv_conf_nameserver_classes'] = servers
code, text = read(['nmcli', '-t', '-f', 'TYPE', 'connection', 'show', '--active'])
report['nmcli_exit'] = code
report['active_connection_types'] = sorted(set(text.splitlines())) if code == 0 else []
code, text = read(['resolvectl', 'domain'])
report['resolved_domain_query_exit'] = code
records = []
if code == 0:
    for line in text.splitlines():
        _, separator, domains = line.partition(':')
        if separator:
            items = domains.split()
            records.append({'scope': 'global' if line.startswith('Global:') else 'link',
                            'routing_domains': sum(x.startswith('~') for x in items),
                            'search_domains': sum(not x.startswith('~') for x in items),
                            'route_all': '~.' in items})
report['domain_routing_counts'] = records
code, text = read(['ip', '-json', '-details', 'link', 'show'])
report['ip_query_exit'] = code
counts = {}
if code == 0:
    for link in json.loads(text):
        kind = link.get('linkinfo', {}).get('info_kind', 'unspecified')
        counts[kind] = counts.get(kind, 0) + 1
report['link_kind_counts'] = counts
with args.out.open('x') as stream:
    json.dump(report, stream, indent=2)
    stream.write('\n')
print('HOST_RESOLVER_PROFILE_RECORDED')
