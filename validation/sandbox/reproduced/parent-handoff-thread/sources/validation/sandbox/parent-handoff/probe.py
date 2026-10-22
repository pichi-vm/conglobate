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
import threading

if sys.argv[1:2] == ['--parent']:
    fd = os.pidfd_open(os.getpid())
    mode = sys.argv[3]
    children = []
    def spawn():
        if mode == 'nested':
            assert ctypes.CDLL(None, use_errno=True).unshare(0x20000000) == 0
        child = subprocess.Popen(['/parent-handoff', sys.argv[2], mode], stdin=fd)
        children.append(child)
        Path('/work/child').write_text(str(child.pid))
        if sys.argv[2].startswith('thread-'):
            deadline = time.monotonic() + 5
            while not Path('/work/exit-thread').exists():
                assert time.monotonic() < deadline
                time.sleep(.01)
    if sys.argv[2].startswith('thread-'):
        thread = threading.Thread(target=spawn)
        thread.start()
        thread.join(timeout=7)
        assert not thread.is_alive()
        Path('/work/thread-exited').touch()
    else:
        spawn()
    os.close(fd)
    result = children[0].wait(timeout=10)
    raise SystemExit(result if result >= 0 else 128 - result)

assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0
for mode, stage in itertools.product(('same', 'nested'),
                                     ('before-arm', 'after-arm', 'after-check', 'live-control',
                                      'exec-after-check', 'credential-before-arm',
                                      'credential-after-check', 'credential-cleared',
                                      'credential-live', 'thread-before-arm', 'thread-after-arm')):
    for name in ('stopped', 'admitted', 'finished', 'child', 'identity', 'resume', 'exit-thread', 'thread-exited'):
        Path('/work', name).unlink(missing_ok=True)
    selected = {'live-control': 'after-check',
                'credential-live': 'credential-after-check'}.get(stage, stage)
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
        if stage.startswith('thread-'):
            Path('/work/exit-thread').touch()
            deadline = time.monotonic() + 5
            while not Path('/work/thread-exited').exists():
                assert time.monotonic() < deadline
                time.sleep(.01)
            # The creating thread is gone; the parent process remains alive.
            pp = select.poll()
            pp.register(parent_fd, select.POLLIN)
            assert not pp.poll(0)
            if stage == 'thread-before-arm':
                Path('/work/resume').touch()
                signal.pidfd_send_signal(child_fd, signal.SIGCONT)
                assert parent.wait(timeout=5) == 0
                assert Path('/work/admitted').exists() and Path('/work/finished').exists()
                print('PARENT_THREAD_PREARM_PROCESS_PIDFD_LIMITATION', mode, flush=True)
            else:
                assert parent.wait(timeout=5) == 128 + signal.SIGKILL
                assert not Path('/work/admitted').exists()
                assert not Path('/work/finished').exists()
                print('PARENT_THREAD_POSTARM_DEATH_CONFIRMED', mode, flush=True)
        elif stage in ('live-control', 'credential-live'):
            Path('/work/resume').touch()
            signal.pidfd_send_signal(child_fd, signal.SIGCONT)
            assert parent.wait(timeout=5) == 0
            assert Path('/work/admitted').is_file() and Path('/work/finished').is_file()
        else:
            signal.pidfd_send_signal(parent_fd, signal.SIGKILL)
            assert parent.wait(timeout=5) == -signal.SIGKILL
            if stage in ('before-arm', 'credential-before-arm'):
                Path('/work/resume').touch()
                signal.pidfd_send_signal(child_fd, signal.SIGCONT)
            poller = select.poll()
            poller.register(child_fd, select.POLLIN)
            if stage == 'credential-cleared':
                assert not poller.poll(100), 'cleared link unexpectedly killed child'
                Path('/work/resume').touch()
                signal.pidfd_send_signal(child_fd, signal.SIGCONT)
            assert poller.poll(3000), stage
            reaped, status = os.waitpid(pid, 0)
            assert reaped == pid
            if stage in ('before-arm', 'credential-before-arm'):
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 1
                assert not Path('/work/admitted').exists()
            elif stage == 'credential-cleared':
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0
            else:
                assert os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGKILL
            assert Path('/work/finished').exists() == (stage == 'credential-cleared')
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
