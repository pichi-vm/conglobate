#!/usr/bin/python3
"""Bounded parent-death handoff checks, with an inherited pinned parent pidfd."""
import ctypes
import itertools
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

if sys.argv[1:2] == ['--parent']:
    fd = os.pidfd_open(os.getpid())
    mode = sys.argv[3]
    if mode == 'nested':
        assert ctypes.CDLL(None, use_errno=True).unshare(0x20000000) == 0
    child = subprocess.Popen(['/parent-handoff', sys.argv[2], mode], stdin=fd)
    os.close(fd)
    Path('/work/child').write_text(str(child.pid))
    result = child.wait(timeout=10)
    raise SystemExit(result if result >= 0 else 128 - result)

assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
for mode, stage in itertools.product(('same', 'nested'),
                                     ('before-arm', 'after-arm', 'after-check', 'live-control')):
    for name in ('stopped', 'admitted', 'finished', 'child', 'identity', 'resume'):
        Path('/work', name).unlink(missing_ok=True)
    selected = 'after-check' if stage == 'live-control' else stage
    parent = subprocess.Popen(['python3', '/probe.py', '--parent', selected, mode])
    parent_fd = os.pidfd_open(parent.pid)
    child_fd = None
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            assert parent.poll() is None
            if Path('/work/stopped').exists() and Path('/work/child').exists():
                pid = int(Path('/work/child').read_text())
                if mode == 'nested':
                    os.kill(pid, signal.SIGSTOP)
                if '\nState:\tT' in Path(f'/proc/{pid}/status').read_text():
                    break
            time.sleep(.01)
        else:
            raise AssertionError('handoff stop not reached')
        assert Path('/work/stopped').read_text() == selected
        child_fd = os.pidfd_open(pid)
        identity = Path('/work/identity').read_text().split()
        if mode == 'nested':
            assert identity == ['1', '0'], identity
            status = Path(f'/proc/{pid}/status').read_text()
            assert f'NSpid:\t{pid}\t1\n' in status, status
        else:
            assert identity == [str(pid), str(parent.pid)], identity
        if stage == 'live-control':
            Path('/work/resume').touch()
            signal.pidfd_send_signal(child_fd, signal.SIGCONT)
            assert parent.wait(timeout=5) == 0
            assert Path('/work/admitted').is_file() and Path('/work/finished').is_file()
        else:
            signal.pidfd_send_signal(parent_fd, signal.SIGKILL)
            assert parent.wait(timeout=5) == -signal.SIGKILL
            if stage == 'before-arm':
                Path('/work/resume').touch()
                signal.pidfd_send_signal(child_fd, signal.SIGCONT)
            poller = select.poll()
            poller.register(child_fd, select.POLLIN)
            assert poller.poll(3000), stage
            reaped, status = os.waitpid(pid, 0)
            assert reaped == pid
            if stage == 'before-arm':
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 1
                assert not Path('/work/admitted').exists()
            else:
                assert os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGKILL
            assert not Path('/work/finished').exists()
        print('SAFE_PARENT_HANDOFF_PASS', stage, flush=True)
        print('SAFE_PARENT_HANDOFF_MODE_PASS', mode, stage, flush=True)
    finally:
        if parent.poll() is None:
            signal.pidfd_send_signal(parent_fd, signal.SIGKILL)
            parent.wait(timeout=5)
        if child_fd is not None:
            try:
                signal.pidfd_send_signal(child_fd, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(child_fd)
        os.close(parent_fd)
print('SAFE_PARENT_HANDOFF_MATRIX_PASS', flush=True)
