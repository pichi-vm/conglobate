// SPDX-License-Identifier: Apache-2.0
/* Benign metadata assertions on fixture files only, inside disposable VM. */
#define _GNU_SOURCE
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/xattr.h>
#include <unistd.h>
int main(int argc, char **argv) {
    assert(!access("/run/global-label-fixture", F_OK));
    assert(argc == 2 && (!strcmp(argv[1], "on") || !strcmp(argv[1], "off")));
    FILE *status = fopen("/proc/self/status", "r"); assert(status);
    char line[512]; unsigned long long caps = ~0ULL; unsigned long outer = 0;
    while (fgets(line, sizeof(line), status))
        if (!strncmp(line, "CapEff:", 7)) assert(sscanf(line + 7, "%llx", &caps) == 1);
    fclose(status); assert(caps == 0);
    FILE *mapping = fopen("/proc/self/uid_map", "r"); assert(mapping);
    unsigned long inner, count;
    assert(fscanf(mapping, "%lu %lu %lu", &inner, &outer, &count) == 3);
    fclose(mapping); assert(inner == 0 && outer == 65536 && count == 1);
    const char label[] = "foreign_u:foreign_r:foreign_t:s0";
    int enabled = !strcmp(argv[1], "on");
    errno = 0;
    int result = setxattr("/assigned/file", "security.selinux", label, sizeof(label), 0);
    if (!enabled) { assert(result == -1 && errno == EPERM); puts("NO_POLICY_DENIED"); return 0; }
    assert(result == 0);
    char value[128] = {0};
    assert(getxattr("/assigned/file", "security.selinux", value, sizeof(value)) == sizeof(label));
    assert(!memcmp(value, label, sizeof(label)));
    assert(!removexattr("/assigned/file", "security.selinux"));
    /* Admission-probe candidate: no pathname or reusable output artifact. */
    int anonymous = open("/assigned", O_TMPFILE | O_RDWR | O_EXCL | O_CLOEXEC, 0600);
    assert(anonymous >= 0);
    assert(!fsetxattr(anonymous, "security.selinux", label, sizeof(label), 0));
    memset(value, 0, sizeof(value));
    assert(fgetxattr(anonymous, "security.selinux", value, sizeof(value)) == sizeof(label));
    assert(!memcmp(value, label, sizeof(label)));
    assert(!fremovexattr(anonymous, "security.selinux"));
    assert(!close(anonymous));
    puts("ANONYMOUS_LABEL_PROBE_PASS");
    /* Ordinary write-mode denial does not protect security.* labels. */
    assert(!setxattr("/assigned/read-only-mode", "security.selinux", label, sizeof(label), 0));
    errno = 0;
    assert(setxattr("/readonly/file", "security.selinux", label, sizeof(label), 0) == -1 && errno == EROFS);
    const char *names[] = {"security.selinux.extra", "security.other", "trusted.fixture"};
    for (unsigned int i = 0; i < sizeof(names)/sizeof(names[0]); ++i) {
        errno = 0;
        assert(setxattr("/assigned/file", names[i], "x", 1, 0) == -1 && errno == EPERM);
    }
    /* Valid revision-2 file-capability bytes; no capability authorization. */
    unsigned char capability[20] = {0}; capability[3] = 2; capability[4] = 1;
    errno = 0;
    assert(setxattr("/assigned/file", "security.capability", capability, sizeof(capability), 0) == -1 && errno == EPERM);
    puts("GLOBAL_EXACT_LABEL_BOUNDARY_PASS");
    return 0;
}
