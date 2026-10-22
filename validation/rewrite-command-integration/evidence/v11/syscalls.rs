// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use linux_raw_sys::general as sys;
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};
use std::{collections::BTreeMap, fs, io};

/// Compiled native-ABI command filter. Default-deny candidate derived from the
/// preserved syscall validation, not a qualified distro compatibility contract.
/// No policy file or recipe can add authority. Other ABIs are rejected by the
/// generated architecture guard (x32 numbers also miss the native allowlist).
#[derive(Debug)]
pub struct CommandSyscalls {
    fallback: BpfProgram,
    allowed: BpfProgram,
}
impl CommandSyscalls {
    pub fn compile() -> io::Result<Self> {
        if !cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            return Err(io::Error::other(
                "command syscall policy supports amd64/arm64 only",
            ));
        }
        let arch = std::env::consts::ARCH
            .try_into()
            .map_err(io::Error::other)?;
        let mut rules: BTreeMap<i64, Vec<SeccompRule>> = Self::ordinary()
            .iter()
            .map(|&nr| (i64::from(nr), Vec::new()))
            .collect();
        #[cfg(target_arch = "x86_64")]
        rules.extend(Self::legacy().iter().map(|&nr| (i64::from(nr), Vec::new())));
        // clone flags are scalar argument zero on amd64 and arm64. Block every
        // namespace flag, including NEWTIME; ordinary process/thread clones work.
        rules.insert(
            i64::from(sys::__NR_clone),
            vec![
                SeccompRule::new(vec![
                    SeccompCondition::new(
                        0,
                        SeccompCmpArgLen::Qword,
                        SeccompCmpOp::MaskedEq(0x7e020080),
                        0,
                    )
                    .map_err(io::Error::other)?,
                ])
                .map_err(io::Error::other)?,
            ],
        );
        // clone3's pointer cannot be inspected safely by classic seccomp. The
        // separate fallback filter returns ENOSYS for libc's ordinary fallback.
        rules.insert(i64::from(sys::__NR_clone3), Vec::new());
        rules.insert(
            i64::from(sys::__NR_socket),
            Self::equal_rules(0, &[1, 2, 10])?,
        );
        rules.insert(i64::from(sys::__NR_socketpair), Self::equal_rules(0, &[1])?);
        // Limited prctl queries/names/credential operations. No PR_SET_SECCOMP,
        // PR_SET_MM, namespace controls, or unrestricted prctl pass-through.
        rules.insert(
            i64::from(sys::__NR_prctl),
            Self::equal_rules(0, &[3, 7, 8, 15, 16, 21, 23, 24, 27, 28, 38, 39, 47])?,
        );
        // Terminal/descriptor and filesystem flag queries, not mutations.
        rules.insert(
            i64::from(sys::__NR_ioctl),
            Self::equal_rules(
                1,
                &[
                    0x5401, 0x802c542a, 0x540f, 0x5413, 0x541b, 0x5451, 0x5450, 0x80086601,
                ],
            )?,
        );
        let allowed = SeccompFilter::new(
            rules,
            SeccompAction::Errno(rustix::io::Errno::PERM.raw_os_error() as u32),
            SeccompAction::Allow,
            arch,
        )
        .map_err(io::Error::other)?
        .try_into()
        .map_err(io::Error::other)?;
        let fallback = SeccompFilter::new(
            BTreeMap::from([(i64::from(sys::__NR_clone3), Vec::new())]),
            SeccompAction::Allow,
            SeccompAction::Errno(rustix::io::Errno::NOSYS.raw_os_error() as u32),
            arch,
        )
        .map_err(io::Error::other)?
        .try_into()
        .map_err(io::Error::other)?;
        Ok(Self { fallback, allowed })
    }

    /// Apply only in the single-threaded trusted command setup process, after
    /// privilege finalization, mounts and descriptor setup. Failure can leave
    /// no_new_privs or the fallback filter installed: exit the helper, never
    /// execute a command after failure. Installation itself is not admission.
    pub fn apply(self) -> io::Result<()> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("syscall setup requires one thread"));
        }
        seccompiler::apply_filter(&self.fallback).map_err(io::Error::other)?;
        seccompiler::apply_filter(&self.allowed).map_err(io::Error::other)
    }

    fn equal_rules(argument: u8, values: &[u64]) -> io::Result<Vec<SeccompRule>> {
        values
            .iter()
            .map(|&value| {
                let condition = SeccompCondition::new(
                    argument,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Eq,
                    value,
                )
                .map_err(io::Error::other)?;
                SeccompRule::new(vec![condition]).map_err(io::Error::other)
            })
            .collect()
    }

    fn ordinary() -> &'static [u32] {
        &[
            sys::__NR_read,
            sys::__NR_write,
            sys::__NR_readv,
            sys::__NR_writev,
            sys::__NR_pread64,
            sys::__NR_pwrite64,
            sys::__NR_preadv,
            sys::__NR_pwritev,
            sys::__NR_preadv2,
            sys::__NR_pwritev2,
            sys::__NR_close,
            sys::__NR_close_range,
            sys::__NR_lseek,
            sys::__NR_openat,
            sys::__NR_openat2,
            sys::__NR_fstat,
            sys::__NR_newfstatat,
            sys::__NR_statx,
            sys::__NR_faccessat,
            sys::__NR_faccessat2,
            sys::__NR_readlinkat,
            sys::__NR_getdents64,
            sys::__NR_getcwd,
            sys::__NR_chdir,
            sys::__NR_fchdir,
            sys::__NR_mkdirat,
            sys::__NR_unlinkat,
            sys::__NR_renameat,
            sys::__NR_renameat2,
            sys::__NR_linkat,
            sys::__NR_symlinkat,
            sys::__NR_truncate,
            sys::__NR_ftruncate,
            sys::__NR_fallocate,
            sys::__NR_fsync,
            sys::__NR_fdatasync,
            sys::__NR_sync,
            sys::__NR_syncfs,
            sys::__NR_sync_file_range,
            sys::__NR_fchmod,
            sys::__NR_fchmodat,
            sys::__NR_fchmodat2,
            sys::__NR_fchown,
            sys::__NR_fchownat,
            sys::__NR_umask,
            sys::__NR_utimensat,
            sys::__NR_getxattr,
            sys::__NR_lgetxattr,
            sys::__NR_fgetxattr,
            sys::__NR_listxattr,
            sys::__NR_llistxattr,
            sys::__NR_flistxattr,
            sys::__NR_setxattr,
            sys::__NR_lsetxattr,
            sys::__NR_fsetxattr,
            sys::__NR_removexattr,
            sys::__NR_lremovexattr,
            sys::__NR_fremovexattr,
            sys::__NR_getxattrat,
            sys::__NR_listxattrat,
            sys::__NR_setxattrat,
            sys::__NR_removexattrat,
            sys::__NR_mmap,
            sys::__NR_mprotect,
            sys::__NR_munmap,
            sys::__NR_mremap,
            sys::__NR_brk,
            sys::__NR_madvise,
            sys::__NR_mlock,
            sys::__NR_munlock,
            sys::__NR_mlockall,
            sys::__NR_munlockall,
            sys::__NR_rt_sigaction,
            sys::__NR_rt_sigprocmask,
            sys::__NR_rt_sigreturn,
            sys::__NR_rt_sigpending,
            sys::__NR_rt_sigtimedwait,
            sys::__NR_rt_sigsuspend,
            sys::__NR_sigaltstack,
            sys::__NR_exit,
            sys::__NR_exit_group,
            sys::__NR_execve,
            sys::__NR_execveat,
            sys::__NR_chroot,
            sys::__NR_wait4,
            sys::__NR_waitid,
            sys::__NR_getpid,
            sys::__NR_getppid,
            sys::__NR_gettid,
            sys::__NR_getuid,
            sys::__NR_geteuid,
            sys::__NR_getgid,
            sys::__NR_getegid,
            sys::__NR_getresuid,
            sys::__NR_getresgid,
            sys::__NR_getgroups,
            sys::__NR_setuid,
            sys::__NR_setgid,
            sys::__NR_setreuid,
            sys::__NR_setregid,
            sys::__NR_setresuid,
            sys::__NR_setresgid,
            sys::__NR_setfsuid,
            sys::__NR_setfsgid,
            sys::__NR_setgroups,
            sys::__NR_capget,
            sys::__NR_capset,
            sys::__NR_getpgid,
            sys::__NR_setpgid,
            sys::__NR_setsid,
            sys::__NR_getsid,
            sys::__NR_kill,
            sys::__NR_tkill,
            sys::__NR_tgkill,
            sys::__NR_getrlimit,
            sys::__NR_setrlimit,
            sys::__NR_prlimit64,
            sys::__NR_getrusage,
            sys::__NR_times,
            sys::__NR_clock_gettime,
            sys::__NR_clock_getres,
            sys::__NR_clock_nanosleep,
            sys::__NR_nanosleep,
            sys::__NR_gettimeofday,
            sys::__NR_setitimer,
            sys::__NR_getitimer,
            sys::__NR_sched_yield,
            sys::__NR_sched_getaffinity,
            sys::__NR_sched_setaffinity,
            sys::__NR_sched_getparam,
            sys::__NR_sched_getscheduler,
            sys::__NR_sched_get_priority_max,
            sys::__NR_sched_get_priority_min,
            sys::__NR_futex,
            sys::__NR_futex_waitv,
            sys::__NR_set_robust_list,
            sys::__NR_get_robust_list,
            sys::__NR_set_tid_address,
            sys::__NR_rseq,
            sys::__NR_membarrier,
            sys::__NR_pipe2,
            sys::__NR_dup,
            sys::__NR_dup3,
            sys::__NR_fcntl,
            sys::__NR_ppoll,
            sys::__NR_pselect6,
            sys::__NR_epoll_create1,
            sys::__NR_epoll_ctl,
            sys::__NR_epoll_pwait,
            sys::__NR_epoll_pwait2,
            sys::__NR_eventfd2,
            sys::__NR_signalfd4,
            sys::__NR_timerfd_create,
            sys::__NR_timerfd_settime,
            sys::__NR_timerfd_gettime,
            sys::__NR_sendto,
            sys::__NR_recvfrom,
            sys::__NR_sendmsg,
            sys::__NR_recvmsg,
            sys::__NR_sendmmsg,
            sys::__NR_recvmmsg,
            sys::__NR_bind,
            sys::__NR_connect,
            sys::__NR_listen,
            sys::__NR_accept,
            sys::__NR_accept4,
            sys::__NR_shutdown,
            sys::__NR_getsockname,
            sys::__NR_getpeername,
            sys::__NR_setsockopt,
            sys::__NR_getsockopt,
            sys::__NR_uname,
            sys::__NR_sysinfo,
            sys::__NR_getrandom,
            sys::__NR_restart_syscall,
            sys::__NR_statfs,
            sys::__NR_fstatfs,
            sys::__NR_flock,
            sys::__NR_fadvise64,
            sys::__NR_readahead,
            sys::__NR_sendfile,
            sys::__NR_copy_file_range,
            sys::__NR_splice,
            sys::__NR_tee,
            sys::__NR_vmsplice,
        ]
    }

    #[cfg(target_arch = "x86_64")]
    fn legacy() -> &'static [u32] {
        &[
            sys::__NR_open,
            sys::__NR_creat,
            sys::__NR_stat,
            sys::__NR_lstat,
            sys::__NR_access,
            sys::__NR_readlink,
            sys::__NR_getdents,
            sys::__NR_mkdir,
            sys::__NR_rmdir,
            sys::__NR_unlink,
            sys::__NR_rename,
            sys::__NR_link,
            sys::__NR_symlink,
            sys::__NR_chmod,
            sys::__NR_chown,
            sys::__NR_lchown,
            sys::__NR_utime,
            sys::__NR_utimes,
            sys::__NR_futimesat,
            sys::__NR_fork,
            sys::__NR_vfork,
            sys::__NR_getpgrp,
            sys::__NR_time,
            sys::__NR_alarm,
            sys::__NR_pipe,
            sys::__NR_dup2,
            sys::__NR_poll,
            sys::__NR_select,
            sys::__NR_epoll_create,
            sys::__NR_epoll_wait,
            sys::__NR_eventfd,
            sys::__NR_signalfd,
            sys::__NR_arch_prctl,
        ]
    }
}
