#!/usr/bin/python3
"""Deployment feature controls inside a disposable guest, never on the host."""
import errno
import json
import os
from pathlib import Path
import stat
import subprocess
import signal

assert os.getuid() == 0
config = dict(line.split('=', 1) for line in Path('/kernel-config').read_text().splitlines()
              if line.startswith('CONFIG_') and '=' in line)
required = ['NAMESPACES', 'USER_NS', 'PID_NS', 'NET_NS', 'IPC_NS', 'UTS_NS',
            'SECCOMP', 'SECCOMP_FILTER', 'FUSE_FS', 'TMPFS', 'TMPFS_XATTR',
            'TMPFS_POSIX_ACL', 'BLK_DEV_LOOP', 'BLK_DEV_DM', 'DM_SNAPSHOT',
            'DM_VERITY', 'DM_ZERO', 'CGROUPS', 'CGROUP_PIDS', 'MEMCG',
            'VETH', 'NF_TABLES', 'NF_NAT']
selected = {name: config.get('CONFIG_' + name) for name in required}
assert all(value in ('y', 'm') for value in selected.values()), selected
print('DEPLOYMENT_KERNEL_CONFIG ' + json.dumps(selected, sort_keys=True), flush=True)
assert {'memory', 'pids'} <= set(Path('/sys/fs/cgroup/cgroup.controllers').read_text().split())
assert not list(Path('/sys/fs/fuse/connections').iterdir())
for path in ['/dev/fuse', '/dev/loop-control', '/dev/mapper/control']:
    assert stat.S_ISCHR(Path(path).stat().st_mode), path
    fd = os.open(path, os.O_RDWR | os.O_CLOEXEC)
    os.close(fd)
print('DEPLOYMENT_STORAGE_CONTROL_AND_CGROUP_AVAILABILITY_PASS', flush=True)

mapped = ['unshare', '--user', '--mount', '--pid', '--net', '--ipc', '--uts', '--fork',
          '--map-users', '0:100000:65536', '--map-groups', '0:100000:65536',
          '--setuid', '0', '--setgid', '0', '/usr/bin/python3', '-c',
          "from pathlib import Path; import os; assert os.getuid()==0; "
          "assert Path('/proc/self/uid_map').read_text().split()==['0','100000','65536']; "
          "assert Path('/proc/self/gid_map').read_text().split()==['0','100000','65536']"]

def run(command):
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True, start_new_session=True)
    try:
        stdout, stderr = child.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        child.communicate(timeout=2)
        raise
    return subprocess.CompletedProcess(command, child.returncode, stdout, stderr)

assert run(mapped).returncode == 0
limit = Path('/proc/sys/user/max_user_namespaces')
old = limit.read_text()
try:
    limit.write_text('0')
    result = run(['unshare', '--user', '/usr/bin/python3', '-c', 'raise AssertionError("unexpected namespace creation")'])
    assert result.returncode != 0 and 'No space left on device' in result.stderr, result
    print('DEPLOYMENT_ZERO_USERNS_LIMIT_REJECTED_PASS', flush=True)
finally:
    limit.write_text(old)
assert run(mapped).returncode == 0
result = run(['setpriv', '--bounding-set=-setuid,-setgid'] + mapped)
assert result.returncode != 0 and 'Operation not permitted' in result.stderr, result
print('DEPLOYMENT_MAPPING_AUTHORITY_REQUIRED_PASS', flush=True)

mode = stat.S_IMODE(Path('/dev/fuse').stat().st_mode)
unprivileged = ['setpriv', '--reuid=100000', '--regid=100000', '--clear-groups',
                '/usr/bin/python3', '-c']
try:
    os.chmod('/dev/fuse', 0o600)
    denied = "import os,errno\ntry: os.open('/dev/fuse',os.O_RDWR)\nexcept OSError as e: assert e.errno==errno.EACCES\nelse: raise AssertionError('device permission bypass')"
    result = run(unprivileged + [denied])
    assert result.returncode == 0, result
    os.chmod('/dev/fuse', 0o666)
    result = run(unprivileged + ["import os; fd=os.open('/dev/fuse',os.O_RDWR); os.close(fd)"])
    assert result.returncode == 0, result
finally:
    os.chmod('/dev/fuse', mode)
assert not list(Path('/sys/fs/fuse/connections').iterdir())
assert limit.read_text() == old
print('DEPLOYMENT_FUSE_DEVICE_PERMISSION_CONTROLS_PASS', flush=True)
print('DEPLOYMENT_PREREQUISITE_MATRIX_PASS', flush=True)
