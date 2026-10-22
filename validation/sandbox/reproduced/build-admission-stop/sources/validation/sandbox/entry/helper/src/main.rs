//! Disposable launcher-ordering probe, not the production executor.
#![forbid(unsafe_code)]
use nix::{
    mount::{MntFlags, umount2},
    unistd::{chdir, close, execve, pivot_root},
};
use std::{error::Error, ffi::CString};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("root argument missing")?;
    let command = args.next().ok_or("command argument missing")?;
    let restricted = match args.next().as_deref() {
        Some("--restricted") => true,
        None => false,
        Some(_) => return Err("unknown policy argument".into()),
    };
    if args.next().is_some() {
        return Err("extra arguments".into());
    }
    // Enumerate while proc and executable are still trusted. Drop the iterator
    // before closing descriptors. This is a single-threaded disposable helper.
    let fds: Vec<i32> = std::fs::read_dir("/proc/self/fd")?
        .map(|entry| -> Result<i32, Box<dyn Error>> {
            Ok(entry?.file_name().to_str().ok_or("non-UTF8 FD")?.parse()?)
        })
        .collect::<Result<_, _>>()?;
    chdir(root.as_str())?;
    pivot_root(".", ".old")?;
    chdir("/")?;
    umount2("/.old", MntFlags::MNT_DETACH)?;
    for fd in fds.into_iter().filter(|fd| *fd >= 3) {
        // The directory iterator's FD is already closed.
        match close(fd) {
            Ok(()) | Err(nix::errno::Errno::EBADF) => (),
            Err(error) => return Err(error.into()),
        }
    }
    if restricted {
        restrictions()?;
    }
    let argv = [CString::new(command)?];
    let env = [
        CString::new("PATH=/usr/sbin:/usr/bin:/sbin:/bin")?,
        CString::new("HOME=/")?,
    ];
    execve(&argv[0], &argv, &env).map_err(|error| {
        format!(
            "exec {} after isolation: {error}",
            argv[0].to_string_lossy()
        )
    })?;
    Ok(())
}

// Candidate policy only: exercising these primitives does not qualify the
// policy for every distribution or constitute a complete syscall allowlist.
fn restrictions() -> Result<(), Box<dyn Error>> {
    use caps::{CapSet, Capability::*};
    use nix::libc;
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule,
    };
    let keep = [
        CAP_CHOWN,
        CAP_DAC_OVERRIDE,
        CAP_FOWNER,
        CAP_FSETID,
        CAP_KILL,
        CAP_SETGID,
        CAP_SETUID,
        CAP_SETPCAP,
        CAP_NET_BIND_SERVICE,
        // Needed by VFS for security.selinux writes on the owning-userns
        // FUSE mount even with the guest SELinux LSM disabled.
        CAP_SYS_ADMIN,
        CAP_SYS_CHROOT,
        CAP_SETFCAP,
    ]
    .into_iter()
    .collect();
    for cap in caps::read(None, CapSet::Bounding)?.difference(&keep) {
        caps::drop(None, CapSet::Bounding, *cap)?;
    }
    caps::clear(None, CapSet::Ambient)?;
    caps::clear(None, CapSet::Inheritable)?;
    caps::set(None, CapSet::Effective, &keep)?;
    caps::set(None, CapSet::Permitted, &keep)?;
    nix::sys::prctl::set_no_new_privs()?;
    let mut rules = [
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_fsopen,
        libc::SYS_fspick,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_move_mount,
        libc::SYS_open_tree,
        libc::SYS_mount_setattr,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_getfd,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_userfaultfd,
        libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_reboot,
        libc::SYS_swapon,
        libc::SYS_swapoff,
        libc::SYS_io_uring_setup,
    ]
    .into_iter()
    .map(|call| (call, vec![]))
    .collect::<std::collections::BTreeMap<_, _>>();
    // libc 0.2.191 lacks SYS_open_tree_attr. Linux's x86_64 syscall
    // table and asm-generic UAPI both assign 467; retain the explicit ABI
    // scope rather than assuming numbers are portable to other architectures.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    rules.insert(467, vec![]);
    // Permit ordinary fork/thread clone flags, reject creation of namespaces.
    let ns_bits = [
        libc::CLONE_NEWUSER,
        libc::CLONE_NEWNS,
        libc::CLONE_NEWPID,
        libc::CLONE_NEWNET,
        libc::CLONE_NEWIPC,
        libc::CLONE_NEWUTS,
        libc::CLONE_NEWCGROUP,
    ];
    let clone_rules = ns_bits
        .into_iter()
        .map(|bit| {
            SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Qword,
                SeccompCmpOp::MaskedEq(bit as u64),
                bit as u64,
            )?])
        })
        .collect::<Result<Vec<_>, _>>()?;
    rules.insert(libc::SYS_clone, clone_rules);
    // A private IP namespace is not a socket-family policy. Keep local IPC,
    // ordinary IP and route queries; do not expose unrelated kernel interfaces
    // or optional VM transports. The syscall arguments are C ints.
    let int_condition = |index, comparison, value: i32| {
        SeccompCondition::new(
            index,
            SeccompCmpArgLen::Dword,
            comparison,
            value as u32 as u64,
        )
    };
    let unknown_family = [
        libc::AF_UNIX,
        libc::AF_INET,
        libc::AF_INET6,
        libc::AF_NETLINK,
    ]
    .into_iter()
    .map(|family| int_condition(0, SeccompCmpOp::Ne, family))
    .collect::<Result<Vec<_>, _>>()?;
    rules.insert(
        libc::SYS_socket,
        vec![
            SeccompRule::new(unknown_family)?,
            SeccompRule::new(vec![
                int_condition(0, SeccompCmpOp::Eq, libc::AF_NETLINK)?,
                int_condition(2, SeccompCmpOp::Ne, libc::NETLINK_ROUTE)?,
            ])?,
            SeccompRule::new(vec![
                int_condition(0, SeccompCmpOp::Ne, libc::AF_NETLINK)?,
                int_condition(1, SeccompCmpOp::MaskedEq(0xf), libc::SOCK_RAW)?,
            ])?,
        ],
    );
    rules.insert(
        libc::SYS_socketpair,
        vec![SeccompRule::new(vec![int_condition(
            0,
            SeccompCmpOp::Ne,
            libc::AF_UNIX,
        )?])?],
    );
    let arch = std::env::consts::ARCH.try_into()?;
    let policy: BpfProgram = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        arch,
    )?
    .try_into()?;
    // x32 shares AUDIT_ARCH_X86_64. The crate checks audit architecture but
    // does not reject the x32 syscall bit, so a native-number denylist needs
    // this explicit ABI gate. seccomp_data.nr is the first u32.
    #[cfg(target_arch = "x86_64")]
    let policy: BpfProgram = [
        seccompiler::sock_filter {
            code: (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16,
            jt: 0,
            jf: 0,
            k: 0,
        },
        seccompiler::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K) as u16,
            jt: 0,
            jf: 1,
            k: 0x4000_0000,
        },
        seccompiler::sock_filter {
            code: (libc::BPF_RET | libc::BPF_K) as u16,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        },
    ]
    .into_iter()
    .chain(policy)
    .collect();
    // clone3 arguments are indirect; ENOSYS permits libc's legacy clone fallback.
    let clone3: BpfProgram = SeccompFilter::new(
        [(libc::SYS_clone3, vec![])].into_iter().collect(),
        SeccompAction::Allow,
        SeccompAction::Errno(libc::ENOSYS as u32),
        arch,
    )?
    .try_into()?;
    seccompiler::apply_filter(&policy)?;
    seccompiler::apply_filter(&clone3)?;
    Ok(())
}
