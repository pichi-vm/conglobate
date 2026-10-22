/* Diagnostic-only wrapper for owned disposable images. No failed media write. */
#include <errno.h>
#include <stdatomic.h>
#include <stdio.h>
#include "ext2fs/ext2fs.h"

errcode_t __real_ext2fs_flush2(ext2_filsys fs, int flags);

errcode_t __wrap_ext2fs_flush2(ext2_filsys fs, int flags)
{
    static atomic_int injected;
    errcode_t result = __real_ext2fs_flush2(fs, flags);
    if (result)
        return result;
    /* In the pinned worker, only op_release uses exactly this flag. */
    if (flags == EXT2_FLAG_FLUSH_NO_SYNC && !atomic_exchange(&injected, 1)) {
        fprintf(stderr, "DIAGNOSTIC_RELEASE_ONLY_EIO\n");
        return EIO;
    }
    return 0;
}
