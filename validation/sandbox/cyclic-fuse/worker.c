/* Test-only single-thread callback dependency. Run only in the disposable VM. */
#define FUSE_USE_VERSION 31
#include <fuse.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static int metadata(const char *path, struct stat *st, struct fuse_file_info *fi)
{
    (void)fi;
    memset(st, 0, sizeof(*st));
    st->st_uid = getuid(); st->st_gid = getgid(); st->st_nlink = 1;
    if (!strcmp(path, "/")) { st->st_mode = S_IFDIR | 0755; return 0; }
    if (!strcmp(path, "/file")) { st->st_mode = S_IFREG | 0444; st->st_size = 1; return 0; }
    return -ENOENT;
}
static int read_file(const char *path, char *buffer, size_t size, off_t offset,
                     struct fuse_file_info *fi)
{
    (void)path; (void)buffer; (void)size; (void)offset; (void)fi;
    int marker = open("/work/callback-entered", O_CREAT | O_WRONLY, 0600);
    if (marker < 0) return -errno;
    close(marker);
    /* This callback intentionally depends on the same single-thread service.
       The independent controller observes the queue and aborts the connection. */
    int nested = open("/work/view/file", O_RDONLY);
    int error;
    if (nested < 0) error = errno;
    else {
        char byte;
        ssize_t result = pread(nested, &byte, 1, 0);
        error = result < 0 ? errno : 0;
        close(nested);
    }
    FILE *log = fopen("/work/nested-error", "w");
    if (log) { fprintf(log, "%d\n", error); fclose(log); }
    return error ? -error : -EIO;
}
int main(int argc, char **argv)
{
    const struct fuse_operations ops = { .getattr = metadata, .read = read_file };
    return fuse_main(argc, argv, &ops, NULL);
}
