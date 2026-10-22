/* Bounded descriptor-limit probe, executed only in the disposable guest. */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/resource.h>
#include <unistd.h>
int main(void)
{
    if (getpid() != 1 || getuid() != 0) return 1;
    struct rlimit limit;
    if (getrlimit(RLIMIT_NOFILE, &limit) || limit.rlim_cur != 32 || limit.rlim_max != 32) return 2;
    FILE *file = fopen("/derived", "w");
    if (!file) return 3;
    fputs("partial build before descriptor failure\n", file);
    if (fclose(file)) return 4;
    int descriptors[40];
    int count = 0, confirmed = 0;
    for (; count < 40; count++) {
        descriptors[count] = open("/inherited", O_RDONLY);
        if (descriptors[count] < 0) { confirmed = errno == EMFILE; break; }
    }
    for (int i = 0; i < count; i++) close(descriptors[i]);
    if (!confirmed) return 5;
    puts("CHAIN_COMMAND_FD_EMFILE_CONFIRMED");
    return 125;
}
