// SPDX-License-Identifier: (GPL-2.0-only OR BSD-2-Clause)
/* Scoped label policy. Loading/enabling requires the anchored lifecycle in
 * SANDBOX-DESIGN.txt. Kernel compilation alone does not establish that lifecycle. */
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>
struct ns_common { unsigned int inum; } __attribute__((preserve_access_index));
struct user_namespace { struct ns_common ns; } __attribute__((preserve_access_index));
struct kuid_t { unsigned int val; } __attribute__((preserve_access_index));
struct cred { struct user_namespace *user_ns; struct kuid_t euid; } __attribute__((preserve_access_index));
struct task_struct { const struct cred *cred; } __attribute__((preserve_access_index));
struct super_block { struct user_namespace *s_user_ns; unsigned int s_dev; } __attribute__((preserve_access_index));
struct inode { struct super_block *i_sb; } __attribute__((preserve_access_index));
struct dentry { struct inode *d_inode; } __attribute__((preserve_access_index));
/* Zero userns leaves the exception disabled until all links are pinned. */
struct config { unsigned int userns; unsigned int root_uid; };
struct target { unsigned int sb_userns; unsigned int dev; };
/* Current stages assign at most two writable filesystems (bootstrap). */
struct {
 __uint(type, BPF_MAP_TYPE_HASH);
 __uint(max_entries, 2);
 __uint(map_flags, BPF_F_RDONLY_PROG);
 __type(key, struct target);
 __type(value, unsigned int);
} targets SEC(".maps");
struct { __uint(type, BPF_MAP_TYPE_ARRAY); __uint(max_entries, 1); __uint(map_flags, BPF_F_RDONLY_PROG); __type(key, unsigned int); __type(value, struct config); } config SEC(".maps");
static __always_inline struct config *subject(void) {
 unsigned int zero=0; struct config *c=bpf_map_lookup_elem(&config,&zero);
 if (!c || !c->userns) return 0;
 struct task_struct *t=(void *)bpf_get_current_task_btf();
 return BPF_CORE_READ(t,cred,user_ns,ns.inum)==c->userns && BPF_CORE_READ(t,cred,euid.val)==c->root_uid ? c : 0;
}
static __always_inline int label(const char *name) {
 /* One extra byte distinguishes an exact name from a truncated prefix. */
 char n[18]={0}; if (bpf_probe_read_kernel_str(n,sizeof(n),name)!=17) return 0;
 return !__builtin_memcmp(n,"security.selinux",17);
}
SEC("lsm/inode_xattr_skipcap")
int BPF_PROG(skip_label_cap,const char *name,int ret) {
 /* Do not erase an earlier LSM's claim. Only the selected namespace opts in. */
 if (ret) return ret;
 return subject() && label(name) ? 1 : 0;
}
static __always_inline int check_target(struct dentry *d,const char *name,int ret) {
 if (ret) return ret;
 struct config *c=subject(); if (!c || !label(name)) return 0;
 struct super_block *sb=BPF_CORE_READ(d,d_inode,i_sb);
 struct target target = {
  .sb_userns = BPF_CORE_READ(sb,s_user_ns,ns.inum),
  .dev = BPF_CORE_READ(sb,s_dev),
 };
 const unsigned int *allowed = bpf_map_lookup_elem(&targets, &target);
 if (!allowed || *allowed != 1) return -1;
 return 0;
}
SEC("lsm/inode_setxattr")
int BPF_PROG(check_set,void *idmap,struct dentry *d,const char *name,const void *value,unsigned long size,int flags,int ret) {
 return check_target(d,name,ret);
}
SEC("lsm/inode_removexattr")
int BPF_PROG(check_remove,void *idmap,struct dentry *d,const char *name,int ret) {
 return check_target(d,name,ret);
}
char LICENSE[] SEC("license")="Dual BSD/GPL";
