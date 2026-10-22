#!/usr/bin/python3
"""Owned-file automatic timestamp observations; native Linux statx ABI only."""
import ctypes
import json
import os
from pathlib import Path
import struct
import sys
import time

libc = ctypes.CDLL(None, use_errno=True)
libc.statx.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int,
                      ctypes.c_uint, ctypes.c_void_p]
libc.statx.restype = ctypes.c_int


def observe(path):
    buf = ctypes.create_string_buffer(256)
    descriptor = isinstance(path, int)
    fd, name = (path, b'') if descriptor else (-100, os.fsencode(path))
    flags = 0x100 | 0x2000 | (0x1000 if descriptor else 0)
    if libc.statx(fd, name, flags, 0xfff, buf):
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))
    mask = struct.unpack_from('=I', buf)[0]
    result = {}
    for name, offset, bit in [('atime', 64, 0x20), ('btime', 80, 0x800),
                               ('ctime', 96, 0x80), ('mtime', 112, 0x40)]:
        if mask & bit:
            seconds, nanos = struct.unpack_from('=qI', buf, offset)
            assert nanos < 1_000_000_000
            result[name] = seconds * 1_000_000_000 + nanos
    assert all(name in result for name in ['atime', 'ctime', 'mtime'])
    return result


def validate_result(text, expect_birthtime=False):
    lines = text.splitlines()
    observations = [json.loads(line.removeprefix('AUTOMATIC_TIMESTAMPS_OBSERVATIONS '))
                    for line in lines if line.startswith('AUTOMATIC_TIMESTAMPS_OBSERVATIONS ')]
    remounts = [json.loads(line.removeprefix('AUTOMATIC_TIMESTAMPS_REMOUNT '))
                for line in lines if line.startswith('AUTOMATIC_TIMESTAMPS_REMOUNT ')]
    assert len(observations) == len(remounts) == 6
    for index, (observed, remount) in enumerate(zip(observations, remounts)):
        expected_birthtime = expect_birthtime or index % 3 == 0
        assert observed['birthtime_exposed'] is expected_birthtime
        assert len(observed['checks']) == (10 if expected_birthtime else 9)
        assert all(value is True for value in observed['checks'].values())
        assert remount['before'] == observed['final']
        assert remount['differences'] == {}
        assert all(remount['after'].get(k) == v for k, v in observed['final'].items())
        assert 0 < remount['after']['btime'] <= remount['after']['mtime']
    assert lines.count('AUTOMATIC_TIMESTAMPS_SEMANTICS_PASS') == 6
    assert lines.count('AUTOMATIC_TIMESTAMPS_PERSISTED_PASS') == 6
    if expect_birthtime:
        assert lines.count('BIRTHTIME_INTERFACE_PASS') == 6
        assert lines.count('SMALL_INODE_NO_BIRTHTIME_PASS') == 3
        assert lines.count('SMALL_INODE_KERNEL_REMOUNT_PASS') == 3
    return {'configurations': 6, 'automatic_update_checks': 54,
            'birthtime_stability_checks': 6 if expect_birthtime else 2, 'persisted_comparisons': 6,
            'fuse_birthtime_visibility_limitations': 0 if expect_birthtime else 4}


def main():
    action, root_arg, report_arg = sys.argv[1:]
    root, report = Path(root_arg), Path(report_arg)
    if action in ('small-inode', 'small-inode-verify'):
        path = root / 'small-inode-fixture'
        if action == 'small-inode':
            with path.open('xb') as stream:
                stream.write(b'owned small inode fixture\n')
                stream.flush()
                os.fsync(stream.fileno())
            by_path = observe(path)
            with path.open('rb') as stream:
                assert observe(stream.fileno()) == by_path
                assert stream.read() == b'owned small inode fixture\n'
            actual = observe(path)
            assert 'btime' not in actual
            assert all(value % 1_000_000_000 == 0 for value in actual.values())
            report.write_text(json.dumps(actual) + '\n')
            print('SMALL_INODE_NO_BIRTHTIME_PASS', flush=True)
        else:
            assert path.read_bytes() == b'owned small inode fixture\n'
            actual = observe(path)
            assert 'btime' not in actual
            assert actual == json.loads(report.read_text())
            print('SMALL_INODE_KERNEL_REMOUNT_PASS', flush=True)
        return
    path = root / 'auto-times-renamed'
    if action == 'verify':
        saved = json.loads(report.read_text())
        actual = observe(path)
        expected = saved['final']
        differences = {key: [value, actual.get(key)] for key, value in expected.items()
                       if actual.get(key) != value}
        print('AUTOMATIC_TIMESTAMPS_REMOUNT ' + json.dumps({
            'differences': differences, 'before': expected, 'after': actual}), flush=True)
        if differences:
            print('AUTOMATIC_TIMESTAMPS_PERSISTENCE_LIMITATION', flush=True)
        else:
            print('AUTOMATIC_TIMESTAMPS_PERSISTED_PASS', flush=True)
        return
    assert action == 'exercise'
    if Path('/expect-birthtime').exists():
        owned = root / 'statx-open'
        owned.write_bytes(b'owned statx fixture')
        fd = os.open(owned, os.O_RDONLY)
        try:
            by_path, by_fd = observe(owned), observe(fd)
            assert 'btime' in by_path and by_path == by_fd
            owned.unlink()
            assert observe(fd)['btime'] == by_path['btime']
        finally:
            os.close(fd)
        try:
            observe(owned)
        except FileNotFoundError:
            pass
        else:
            raise AssertionError('missing path statx succeeded')
        link = root / 'statx-link'
        link.symlink_to('statx-open')
        try:
            assert observe(link)['btime'] > 0
        finally:
            link.unlink()
        print('BIRTHTIME_INTERFACE_PASS', flush=True)
    path = root / 'auto-times'
    path.write_bytes(b'owned timestamp fixture\n')
    created = observe(path)
    time.sleep(1.1)
    with path.open('ab') as stream:
        stream.write(b'ordinary write\n')
        stream.flush()
        os.fsync(stream.fileno())
    written = observe(path)
    time.sleep(1.1)
    path.chmod(0o640)
    changed = observe(path)
    time.sleep(1.1)
    renamed = path.with_name('auto-times-renamed')
    path.rename(renamed)
    moved = observe(renamed)
    # Force a stale atime, making this check independent of relatime's window.
    os.utime(renamed, ns=(1_000_000_000, moved['mtime']))
    before_read = observe(renamed)
    assert renamed.read_bytes() == b'owned timestamp fixture\nordinary write\n'
    final = observe(renamed)
    checks = {
        'write_updates_mtime': written['mtime'] > created['mtime'],
        'write_updates_ctime': written['ctime'] > created['ctime'],
        'chmod_updates_ctime': changed['ctime'] > written['ctime'],
        'chmod_preserves_mtime': changed['mtime'] == written['mtime'],
        'rename_updates_ctime': moved['ctime'] > changed['ctime'],
        'rename_preserves_mtime': moved['mtime'] == changed['mtime'],
        'read_updates_stale_atime': final['atime'] > before_read['atime'],
        'read_preserves_mtime': final['mtime'] == before_read['mtime'],
        'read_preserves_ctime': final['ctime'] == before_read['ctime'],
    }
    snapshots = [created, written, changed, moved, before_read, final]
    if 'btime' in created:
        checks['birthtime_stable'] = all(s.get('btime') == created['btime'] for s in snapshots)
    result = {'checks': checks, 'birthtime_exposed': 'btime' in created,
              'snapshots': snapshots, 'final': final}
    report.write_text(json.dumps(result, sort_keys=True) + '\n')
    print('AUTOMATIC_TIMESTAMPS_OBSERVATIONS ' + json.dumps(result, sort_keys=True), flush=True)
    print('AUTOMATIC_TIMESTAMPS_SEMANTICS_' + ('PASS' if all(checks.values()) else 'LIMITATION'), flush=True)


if __name__ == '__main__':
    main()
