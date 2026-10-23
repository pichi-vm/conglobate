// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
// Benign test fixture only: run in the disposable VM, never on the host.
#define _GNU_SOURCE
#include <errno.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static int child(void *unused) {
    (void)unused;
    return 0;
}

int main(int argc, char **argv) {
    if (access("/conglobate-disposable-validation-vm", F_OK) != 0 || argc != 2)
        return 2;
    const int filtered = strcmp(argv[1], "filtered") == 0;
    if (!filtered && strcmp(argv[1], "control") != 0)
        return 2;
    // Match no_new_privs in both arms; retain namespace authority so EPERM
    // cannot be explained by the command capability-dropping policy.
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0)
        return 3;
    FILE *status = fopen("/proc/self/status", "r");
    if (status == NULL)
        return 3;
    char line[512];
    unsigned long long effective = 0;
    while (fgets(line, sizeof(line), status) != NULL)
        if (sscanf(line, "CapEff: %llx", &effective) == 1)
            break;
    fclose(status);
    if ((effective & (1ULL << 21)) == 0) // CAP_SYS_ADMIN
        return 4;
    const struct { const char *name; int flags; } cases[] = {
        {"ordinary", 0}, {"mount", CLONE_NEWNS},
        {"cgroup", CLONE_NEWCGROUP}, {"uts", CLONE_NEWUTS},
        {"ipc", CLONE_NEWIPC}, {"user", CLONE_NEWUSER},
        {"pid", CLONE_NEWPID}, {"network", CLONE_NEWNET},
    };
    const size_t stack_size = 1024 * 1024;
    char *stack = malloc(stack_size);
    if (stack == NULL)
        return 5;
    int failed = 0;
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        errno = 0;
        // libc clone invokes legacy clone directly; it does not use clone3.
        // No shared VM/files/threads, and the child only returns zero.
        pid_t pid = clone(child, stack + stack_size, cases[i].flags | SIGCHLD, NULL);
        const int error = errno;
        const int deny = filtered && cases[i].flags != 0;
        int valid = 0;
        if (pid == -1) {
            valid = deny && error == EPERM;
        } else {
            int result = 0;
            pid_t reaped;
            do { reaped = waitpid(pid, &result, 0); } while (reaped == -1 && errno == EINTR);
            valid = !deny && reaped == pid && WIFEXITED(result) && WEXITSTATUS(result) == 0;
        }
        printf("CLONE_PROBE %s %s %s errno=%d\n", argv[1], cases[i].name,
               valid ? "PASS" : "FAIL", pid == -1 ? error : 0);
        failed |= !valid;
    }
    free(stack);
    // CLONE_NEWTIME occupies the legacy clone signal byte, so exercise its
    // supported unshare interface instead. Only this disposable process's
    // namespace-for-children changes; no clocks or offsets are modified.
    struct stat before, after;
    if (stat("/proc/self/ns/time_for_children", &before) != 0)
        return 6;
    errno = 0;
    const int result = unshare(CLONE_NEWTIME);
    const int error = errno;
    if (stat("/proc/self/ns/time_for_children", &after) != 0)
        return 6;
    const int same = before.st_dev == after.st_dev && before.st_ino == after.st_ino;
    const int valid = filtered ? result == -1 && error == EPERM && same
                               : result == 0 && !same;
    printf("UNSHARE_PROBE %s time %s errno=%d\n", argv[1],
           valid ? "PASS" : "FAIL", result == -1 ? error : 0);
    failed |= !valid;
    return failed;
}
