//! Candidate outer syscall restrictions. Emit BPF for bubblewrap; not production policy.
#![forbid(unsafe_code)]
use std::{error::Error,io::Write};
use nix::libc;
use seccompiler::{BpfProgram,SeccompAction,SeccompCmpArgLen,SeccompCmpOp,SeccompCondition,SeccompFilter,SeccompRule,sock_filter};
fn main()->Result<(),Box<dyn Error>> {
    if std::env::consts::ARCH!="x86_64" {return Err("initial probe supports x86_64 only".into());}
    // UML requires ptrace for its own children. PID isolation and dropped outer
    // capabilities restrict targets; blocking ptrace would prevent UML boot.
    let mut rules=[libc::SYS_mount,libc::SYS_umount2,libc::SYS_pivot_root,libc::SYS_fsopen,
        libc::SYS_fspick,libc::SYS_fsconfig,libc::SYS_fsmount,libc::SYS_move_mount,
        libc::SYS_open_tree,libc::SYS_mount_setattr,libc::SYS_unshare,libc::SYS_setns,
        libc::SYS_pidfd_getfd,libc::SYS_keyctl,libc::SYS_add_key,libc::SYS_request_key,
        libc::SYS_bpf,libc::SYS_perf_event_open,libc::SYS_userfaultfd,libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,libc::SYS_init_module,libc::SYS_finit_module,
        libc::SYS_delete_module,libc::SYS_reboot,libc::SYS_swapon,libc::SYS_swapoff,
        libc::SYS_io_uring_setup].into_iter().map(|n|(n,vec![])).collect::<std::collections::BTreeMap<_,_>>();
    let ns=[libc::CLONE_NEWUSER,libc::CLONE_NEWNS,libc::CLONE_NEWPID,libc::CLONE_NEWNET,
        libc::CLONE_NEWIPC,libc::CLONE_NEWUTS,libc::CLONE_NEWCGROUP];
    rules.insert(libc::SYS_clone,ns.into_iter().map(|bit|SeccompRule::new(vec![SeccompCondition::new(0,SeccompCmpArgLen::Qword,SeccompCmpOp::MaskedEq(bit as u64),bit as u64)?])).collect::<Result<Vec<_>,_>>()?);
    let unknown=[libc::AF_UNIX,libc::AF_INET,libc::AF_INET6,libc::AF_NETLINK].into_iter().map(|f|SeccompCondition::new(0,SeccompCmpArgLen::Dword,SeccompCmpOp::Ne,f as u64)).collect::<Result<Vec<_>,_>>()?;
    rules.insert(libc::SYS_socket,vec![SeccompRule::new(unknown)?]);
    let body:BpfProgram=SeccompFilter::new(rules,SeccompAction::Allow,SeccompAction::Errno(libc::EPERM as u32),"x86_64".try_into()?)?.try_into()?;
    // Gate x32 syscall numbers; return ENOSYS for clone3 so libc can fall back.
    let prefix=[
        sock_filter{code:0x20,jt:0,jf:0,k:0},
        sock_filter{code:0x35,jt:0,jf:1,k:0x40000000},
        sock_filter{code:0x06,jt:0,jf:0,k:libc::SECCOMP_RET_ERRNO|libc::EPERM as u32},
        sock_filter{code:0x15,jt:0,jf:1,k:libc::SYS_clone3 as u32},
        sock_filter{code:0x06,jt:0,jf:0,k:libc::SECCOMP_RET_ERRNO|libc::ENOSYS as u32},
    ];
    let mut out=std::io::stdout().lock();
    for instruction in prefix.into_iter().chain(body) {
        out.write_all(&instruction.code.to_ne_bytes())?;
        out.write_all(&[instruction.jt,instruction.jf])?;
        out.write_all(&instruction.k.to_ne_bytes())?;
    }
    Ok(())
}
