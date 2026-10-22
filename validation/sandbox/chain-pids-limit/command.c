/* One fork attempt under an environment-provisioned pids.max=1. Test only. */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>
int main(void)
{
    if (getpid() != 1 || getuid() != 0) return 1;
    FILE *file = fopen("/derived", "w");
    if (!file) return 2;
    fputs("partial build before resource failure\n", file);
    if (fclose(file)) return 3;
    pid_t child = fork();
    if (child < 0) {
        if (errno != EAGAIN) return 4;
        puts("CHAIN_COMMAND_PIDS_EAGAIN_CONFIRMED");
        return 125; /* Real nonzero command result must reject sealing. */
    }
    if (child == 0) _exit(0);
    (void)waitpid(child, NULL, 0);
    return 5; /* The test verifier rejects missing EAGAIN marker. */
}
