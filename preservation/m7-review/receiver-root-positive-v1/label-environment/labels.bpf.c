// SPDX-License-Identifier: (GPL-2.0-only OR BSD-2-Clause)
/* Validation only: environment-owned exact-name exception, no registration. */
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
SEC("lsm/inode_xattr_skipcap")
int BPF_PROG(skip_label_cap, const char *name, int ret) {
    if (ret) return ret;
    char value[18] = {0};
    if (bpf_probe_read_kernel_str(value, sizeof(value), name) != 17) return 0;
    return !__builtin_memcmp(value, "security.selinux", 17);
}
char LICENSE[] SEC("license") = "Dual BSD/GPL";
