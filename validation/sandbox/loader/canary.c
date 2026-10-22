/* SPDX-License-Identifier: Apache-2.0
 * Guest-only instrumentation: write one fixed marker when this library loads.
 * No input inspection, networking, privileged operation or persistent payload.
 */
#include <fcntl.h>
#include <unistd.h>

__attribute__((constructor))
static void record_loading(void)
{
    static const char marker[] = "loaded-before-isolation\n";
    int fd = open("/work/loader-canary", O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (fd < 0)
        _exit(90);
    if (write(fd, marker, sizeof(marker) - 1) != (ssize_t)(sizeof(marker) - 1))
        _exit(91);
    if (close(fd))
        _exit(92);
}
