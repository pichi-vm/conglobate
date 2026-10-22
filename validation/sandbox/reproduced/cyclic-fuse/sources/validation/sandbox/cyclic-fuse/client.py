import errno
from pathlib import Path
try:
    with open('/work/view/file','rb',buffering=0) as file:
        file.read(1)
except OSError as error:
    assert error.errno in (errno.ECONNABORTED,errno.ENOTCONN), error
    Path('/work/client-error').write_text(str(error.errno))
    print('CYCLIC_CLIENT_ABORT_ERROR', error.errno,flush=True)
else:
    raise AssertionError('cyclic read unexpectedly completed')
