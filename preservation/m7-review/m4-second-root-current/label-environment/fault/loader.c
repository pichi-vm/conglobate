// SPDX-License-Identifier: Apache-2.0
/* Trusted disposable-VM loader/observer; never executed on the host. */
#include <bpf/libbpf.h>
#include <bpf/bpf.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "state.h"
int main(int argc, char **argv) {
    if (access("/run/global-label-fixture", F_OK)) return 2;
    if (argc == 2 && !strcmp(argv[1], "observe")) {
        int fd = bpf_obj_get("/sys/fs/bpf/label-fault-state");
        __u32 key = 0;
        struct fault_state value = {0};
        if (fd < 0 || bpf_map_lookup_elem(fd, &key, &value)) return 3;
        close(fd);
        printf("LABEL_FAULT_OBSERVATION %u %u %u\n", value.first_device, value.allowed, value.denied);
        return 0;
    }
    if (argc != 1) return 4;
    struct bpf_object *object = bpf_object__open_file("/label-fault.bpf.o", NULL);
    if (libbpf_get_error(object) || bpf_object__load(object)) return 5;
    struct bpf_program *program = bpf_object__find_program_by_name(object, "deny_second_probe");
    struct bpf_map *state = bpf_object__find_map_by_name(object, "state");
    if (!program || !state) return 6;
    struct bpf_link *link = bpf_program__attach_lsm(program);
    if (libbpf_get_error(link)) return 7;
    if (bpf_link__pin(link, "/sys/fs/bpf/label-fault-link") ||
        bpf_map__pin(state, "/sys/fs/bpf/label-fault-state")) return 8;
    bpf_link__destroy(link);
    bpf_object__close(object);
    return 0;
}
