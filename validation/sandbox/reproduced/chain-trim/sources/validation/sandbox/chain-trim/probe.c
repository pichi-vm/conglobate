/* Bounded FITRIM compatibility probe for the disposable image root only. */
#include <errno.h>
#include <fcntl.h>
#include <linux/fs.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include <unistd.h>
int main(void) {
    int fd = open("/", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (fd < 0) { perror("open image root"); return 1; }
    struct fstrim_range range = {.start = 0, .len = 64ULL * 1024 * 1024, .minlen = 4096};
    int result = ioctl(fd, FITRIM, &range);
    int error = errno;
    if (close(fd) != 0) { perror("close image root"); return 1; }
    if (result == 0) {
        printf("CHAIN_FITRIM_SUPPORTED reported_len=%llu\n", (unsigned long long)range.len);
    } else if (error == EOPNOTSUPP || error == ENOTTY || error == EPERM) {
        printf("CHAIN_FITRIM_LIMITATION errno=%d\n", error);
    } else {
        fprintf(stderr, "unexpected FITRIM result=%d errno=%d\n", result, error);
        return 1;
    }
    puts("CHAIN_FITRIM_PROBE_COMPLETED");
    return 0;
}
