/* Bounded allocation under a 64MiB command-only memory cgroup. Guest only. */
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
int main(void)
{
    if (getpid() != 1 || getuid() != 0) return 1;
    FILE *file = fopen("/derived", "w");
    if (!file) return 2;
    fputs("partial build before memory limit\n", file);
    if (fclose(file)) return 3;
    puts("CHAIN_COMMAND_MEMORY_PRESSURE_STARTED");
    fflush(stdout);
    /* Volatile writes force physical pages to be charged. Single allocation,
       fixed size; the supervisor and worker are outside this cgroup. */
    volatile unsigned char *memory = malloc(128 * 1024 * 1024);
    if (!memory) return 4;
    for (size_t i = 0; i < 128 * 1024 * 1024; i += 4096) memory[i] = 1;
    free((void *)memory);
    return 5; /* OOM kill and cgroup event are required by the verifier. */
}
