// SPDX-License-Identifier: (GPL-2.0-only OR BSD-2-Clause)
/* Disposable-VM fault injection only. NOT the environment's production policy.
 * Deny the admission probe on a second filesystem, leaving the first working.
 * No userspace registration or conglobate test hook is involved. */
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>
#include "state.h"
struct super_block { unsigned int s_dev; } __attribute__((preserve_access_index));
struct inode { struct super_block *i_sb; } __attribute__((preserve_access_index));
struct dentry { struct inode *d_inode; } __attribute__((preserve_access_index));
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct fault_state);
} state SEC(".maps");
SEC("lsm/inode_setxattr")
int BPF_PROG(deny_second_probe, void *idmap, struct dentry *d,
             const char *name, const void *value, unsigned long size, int flags, int ret) {
    if (ret) return ret;
    const char expected[] = "conglobate_probe_u:conglobate_probe_r:conglobate_probe_t:s0";
    char attribute[18] = {0};
    char label[sizeof(expected)] = {0};
    if (bpf_probe_read_kernel_str(attribute, sizeof(attribute), name) != 17 ||
        __builtin_memcmp(attribute, "security.selinux", 17) || size != sizeof(expected)) return 0;
    if (bpf_probe_read_kernel(label, sizeof(label), value) ||
        __builtin_memcmp(label, expected, sizeof(expected))) return 0;
    __u32 key = 0;
    struct fault_state *record = bpf_map_lookup_elem(&state, &key);
    if (!record) return -1;
    __u32 device = BPF_CORE_READ(d, d_inode, i_sb, s_dev);
    if (!device) return -1;
    __sync_val_compare_and_swap(&record->first_device, 0, device);
    if (record->first_device == device) {
        __sync_fetch_and_add(&record->allowed, 1);
        return 0;
    }
    __sync_fetch_and_add(&record->denied, 1);
    return -1; /* EPERM: deliberately incompatible output-filesystem support. */
}
char LICENSE[] SEC("license") = "Dual BSD/GPL";
