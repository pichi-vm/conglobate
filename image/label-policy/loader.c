// SPDX-License-Identifier: Apache-2.0
/* Trusted disposable-VM loader. Pins once, then exits; no maps or subjects. */
#include <bpf/libbpf.h>
#include <unistd.h>
int main(void) {
    if (access("/run/global-label-fixture", F_OK)) return 2;
    struct bpf_object *object = bpf_object__open_file("/labels.bpf.o", NULL);
    if (libbpf_get_error(object) || bpf_object__load(object)) return 3;
    struct bpf_program *program = bpf_object__find_program_by_name(object, "skip_label_cap");
    if (!program) return 4;
    struct bpf_link *link = bpf_program__attach_lsm(program);
    if (libbpf_get_error(link)) return 5;
    if (bpf_link__pin(link, "/sys/fs/bpf/label-exception")) return 6;
    bpf_link__destroy(link);
    bpf_object__close(object);
    return 0;
}
