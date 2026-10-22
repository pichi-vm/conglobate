#!/usr/bin/python3
"""Diagnostic protected-record recovery; fixture encoding is not a format API."""
import hashlib
import fcntl
import select
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import stat
import subprocess
import sys
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def identity(path):
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode):
        raise ValueError('not a real directory')
    files = {}
    for p in sorted(path.rglob('*')):
        mode = p.lstat().st_mode
        if stat.S_ISREG(mode):
            files[str(p.relative_to(path))] = digest(p)
        elif not stat.S_ISDIR(mode):
            raise ValueError('non-artifact entry')
    return {'device': info.st_dev, 'inode': info.st_ino, 'files': files}


def record(path, value):
    # Same protected directory, no active peer writers. Atomic record install;
    # syncing tmpfs makes no reboot/power-loss durability promise.
    temp = path.with_suffix('.tmp')
    with temp.open('x') as stream:
        json.dump(value, stream, sort_keys=True)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temp, path)
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def recover(root):
    # Caller has reaped the publisher and owns the protected parent. Identity
    # assumes no deletion/reuse/replacement by another trusted actor. This is
    # deliberately not an authentication scheme for arbitrary user directories.
    try:
        prepared = root / 'prepared.json'
        if not stat.S_ISREG(prepared.lstat().st_mode):
            return 'Unknown'
        data = json.loads(prepared.read_text())
        if set(data) != {'version', 'identity'} or data['version'] != 1:
            return 'Unknown'
        expected = data['identity']
        if set(expected) != {'device', 'inode', 'files'}:
            return 'Unknown'
        staging, output = root / 'published', root / 'result'
        has_stage, has_output = os.path.lexists(staging), os.path.lexists(output)
        receipt = root / 'receipt.json'
        has_receipt = os.path.lexists(receipt)
        if has_stage and identity(staging) == expected and not has_receipt:
            return 'Collision' if has_output else 'PreparedNotVisible'
        if not has_stage and has_output and identity(output) == expected:
            if not has_receipt:
                return 'VisibleAcknowledgmentUnknown'
            if not stat.S_ISREG(receipt.lstat().st_mode):
                return 'Unknown'
            value = json.loads(receipt.read_text())
            for phase in ('Committed', 'VisibleError'):
                if value == {'version': 1, 'phase': phase, 'prepared_sha256': digest(prepared)}:
                    return phase
        return 'Unknown'
    except (OSError, ValueError, TypeError):
        return 'Unknown'


def recover_exclusive(root, hold=False):
    try:
        fd = os.open(root / 'recovery.lease', os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
    except OSError:
        return 'Unknown'
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            return 'Unknown'
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return 'Busy'
        if hold:
            print('RECOVERY_LEASE_HELD', flush=True)
            assert select.select([sys.stdin], [], [], 5)[0], 'recovery release timeout'
            assert sys.stdin.readline() == 'continue\n'
        return recover(root)
    finally:
        os.close(fd)


def matrix():
    assert int(Path('/work/owner-status').read_text()) == 0
    source = Path('/work/published')
    expected_source = identity(source)
    assert 'inventory.cbor' in expected_source['files']
    cases = {
        'reserved-death': 'PreparedNotVisible',
        'lost-reply': 'VisibleAcknowledgmentUnknown',
        'receipt': 'Committed',
        'same-bytes-collision': 'Collision',
        'changed-output': 'Unknown',
        'missing-record': 'Unknown',
        'torn-record': 'Unknown',
        'wrong-receipt': 'Unknown',
        'symlink-output': 'Unknown',
        'missing-output': 'Unknown',
        'rename-failure': 'PreparedNotVisible',
        'sync-error-unrecorded': 'VisibleAcknowledgmentUnknown',
        'sync-error-receipt': 'VisibleError',
        'receipt-write-failure': 'VisibleAcknowledgmentUnknown',
        'receipt-replacement-failure': 'Committed',
    }
    for name, expected in cases.items():
        root = Path('/work/recover-' + name)
        root.mkdir(mode=0o700)
        shutil.copytree(source, root / 'published')
        record(root / 'prepared.json', {'version': 1, 'identity': identity(root / 'published')})
        command = ['/commit-arbiter', str(root)]
        if name.startswith('sync-error-'):
            command.append('--path-parent')
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        pidfd = os.pidfd_open(process.pid)
        pending = None
        try:
            def rpc(operation, reply=True):
                stream = socket.socket(socket.AF_UNIX)
                stream.settimeout(3)
                stream.connect(str(root / 'control.sock'))
                stream.sendall(operation.encode() + b'\n')
                if not reply:
                    return stream
                with stream:
                    return stream.makefile('rb').readline(64).decode().strip()
            deadline = time.monotonic() + 5
            while not (root / 'control.sock').exists():
                assert process.poll() is None and time.monotonic() < deadline
                time.sleep(.01)
            assert rpc('reserve') == 'Reserved'
            lease_identity = (root / 'recovery.lease').stat().st_ino
            busy = subprocess.run([sys.executable, __file__, '--recover', str(root)],
                                  capture_output=True, text=True, timeout=5, check=True)
            assert busy.stdout.strip() == 'Busy'
            print('RECOVERY_LIVE_PUBLISHER_EXCLUDED', name, flush=True)
            if name == 'rename-failure':
                # Real ENOENT, preserving original bytes under an owned name.
                (root / 'published').rename(root / 'held-staging')
                assert rpc('commit') == 'Failed'
                assert not (root / 'result').exists()
                (root / 'held-staging').rename(root / 'published')
                assert rpc('commit') == 'Failed'  # No implicit retry after repair.
                assert rpc('cancel') == 'Failed'
                assert not (root / 'result').exists()
            elif name.startswith('sync-error-'):
                assert rpc('commit') == 'VisibleError'
                assert rpc('status') == 'VisibleError'
                assert rpc('cancel') == 'VisibleError'
                assert rpc('commit') == 'VisibleError'
                assert not (root / 'published').exists()
                assert identity(root / 'result')['files'] == expected_source['files']
                if name == 'sync-error-receipt':
                    record(root / 'receipt.json', {'version': 1, 'phase': 'VisibleError',
                           'prepared_sha256': digest(root / 'prepared.json')})
            elif name == 'same-bytes-collision':
                shutil.copytree(source, root / 'result')
                assert identity(root / 'result')['files'] == expected_source['files']
                assert rpc('commit') == 'Collision'
            elif name != 'reserved-death':
                if name == 'lost-reply':
                    pending = rpc('commit-no-reply', False)
                    deadline = time.monotonic() + 3
                    while rpc('status') != 'Committed':
                        assert time.monotonic() < deadline
                        time.sleep(.01)
                else:
                    assert rpc('commit') == 'Committed'
                if name in ('receipt', 'receipt-replacement-failure'):
                    record(root / 'receipt.json', {'version': 1, 'phase': 'Committed',
                           'prepared_sha256': digest(root / 'prepared.json')})
            if name in ('receipt-write-failure', 'receipt-replacement-failure'):
                receipt_path = root / 'receipt.json'
                prior = receipt_path.read_bytes() if receipt_path.exists() else None
                failure = subprocess.run([sys.executable, __file__, '--limited-receipt', str(root)],
                                         capture_output=True, text=True, timeout=5, check=True)
                assert failure.stdout.strip() == 'RECEIPT_WRITE_EFBIG'
                assert not failure.stderr
                assert (root / 'receipt.tmp').stat().st_size == 16
                if prior is None:
                    assert not receipt_path.exists()
                else:
                    assert receipt_path.read_bytes() == prior
                assert identity(root / 'result')['files'] == expected_source['files']
                print('RECEIPT_PARTIAL_TEMP_PRESERVED', name, flush=True)
            signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            process.wait(timeout=3)
            assert process.returncode == -signal.SIGKILL
            if pending is not None:
                assert pending.recv(64) == b''
            if name == 'changed-output':
                (root / 'result/inventory.cbor').write_bytes(b'owned diagnostic change')
            elif name == 'missing-record':
                (root / 'prepared.json').unlink()
            elif name == 'torn-record':
                (root / 'prepared.json').write_text('{')
            elif name == 'wrong-receipt':
                record(root / 'receipt.json', {'version': 1, 'phase': 'Committed', 'prepared_sha256': 'wrong-attempt'})
            elif name == 'symlink-output':
                (root / 'result').rename(root / 'moved-output')
                (root / 'result').symlink_to('moved-output')
            elif name == 'missing-output':
                (root / 'result').rename(root / 'moved-output')
            # A fresh process sees only filesystem records, not the supervisor's
            # observed reply or pre-death in-memory status.
            before = {str(p.relative_to(root)): digest(p) for p in root.rglob('*')
                      if p.is_file() and not p.is_symlink()}
            holder = subprocess.Popen([sys.executable, __file__, '--recover-held', str(root)],
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      text=True)
            try:
                assert select.select([holder.stdout], [], [], 5)[0]
                assert holder.stdout.readline().strip() == 'RECOVERY_LEASE_HELD'
                busy = subprocess.run([sys.executable, __file__, '--recover', str(root)],
                                      capture_output=True, text=True, timeout=5, check=True)
                assert busy.stdout.strip() == 'Busy'
                result, error = holder.communicate('continue\n', timeout=5)
                assert holder.returncode == 0 and not error
                assert result.strip() == expected, (name, result, error)
            finally:
                if holder.poll() is None:
                    holder.kill()
                    holder.wait(timeout=3)
            assert (root / 'recovery.lease').stat().st_ino == lease_identity
            print('RECOVERY_COMPETING_READER_EXCLUDED', name, flush=True)
            after = {str(p.relative_to(root)): digest(p) for p in root.rglob('*')
                     if p.is_file() and not p.is_symlink()}
            assert before == after
            print('RECOVERY_RECORD_CASE_PASS', name, expected, flush=True)
        finally:
            if process.poll() is None:
                signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                process.wait(timeout=3)
            if pending is not None:
                pending.close()
            os.close(pidfd)
            out, err = process.communicate(timeout=3)
            assert not out and not err, (out, err)
        shutil.rmtree(root)
    assert identity(source) == expected_source
    print('RECOVERY_RECORD_MATRIX_PASS', flush=True)
    subprocess.run([sys.executable, '/recovery-lifetime.py'], check=True, timeout=30)


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--recover':
        print(recover_exclusive(Path(sys.argv[2])))
    elif len(sys.argv) == 3 and sys.argv[1] == '--recover-held':
        print(recover_exclusive(Path(sys.argv[2]), hold=True))
    elif len(sys.argv) == 3 and sys.argv[1] == '--limited-receipt':
        # Limit only this owned diagnostic child, never the guest supervisor.
        import errno
        import resource
        root = Path(sys.argv[2])
        value = {'version': 1, 'phase': 'Committed',
                 'prepared_sha256': digest(root / 'prepared.json')}
        signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
        resource.setrlimit(resource.RLIMIT_FSIZE, (16, 16))
        try:
            record(root / 'receipt.json', value)
        except OSError as error:
            assert error.errno == errno.EFBIG, error
            print('RECEIPT_WRITE_EFBIG', flush=True)
        else:
            raise AssertionError('limited receipt unexpectedly succeeded')
    else:
        matrix()
