/* Diagnostic link wrapper, only for disposable fixture binaries. */
#include <errno.h>
#include <stdio.h>
#include "ext2fs/ext2fs.h"

errcode_t __real_ext2fs_close(ext2_filsys fs);

errcode_t __wrap_ext2fs_close(ext2_filsys fs)
{
	errcode_t result = __real_ext2fs_close(fs);
	if (result)
		return result;
	fprintf(stderr, "DIAGNOSTIC_CLOSE_ONLY_EIO\n");
	return EIO;
}
