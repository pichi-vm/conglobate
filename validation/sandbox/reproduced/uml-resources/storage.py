import errno
try:
 with open('/work/finite','wb') as f: f.write(b'x'*(16*1024*1024))
except OSError as e:
 assert e.errno==errno.ENOSPC
 print('ENOSPC_OK',flush=True)
else: raise AssertionError('expected full tmpfs')
