// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
use std::{fs, io, os::unix::net::UnixStream, process::Command};

#[derive(Debug)]
struct CommandProbe;
impl CommandProbe {
    fn control() -> Result<(), Box<dyn std::error::Error>> {
        assert!(std::path::Path::new("/conglobate-disposable-validation-vm").is_file());
        nix::sched::unshare(nix::sched::CloneFlags::empty())?;
        assert_eq!(
            nix::sys::uio::process_vm_readv(nix::unistd::getpid(), &mut [], &[])?,
            0
        );
        let socket = rustix::net::socket_with(
            rustix::net::AddressFamily::NETLINK,
            rustix::net::SocketType::RAW,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )?;
        drop(socket);
        assert!(
            Command::new("/context/probe-runtime/loader")
                .args([
                    "--library-path",
                    "/context/probe-runtime/lib",
                    "/context/probe-runtime/keyctl",
                    "show"
                ])
                .env_clear()
                .env("LC_ALL", "C")
                .output()?
                .status
                .success()
        );
        // Benign VM-owned sentinel, retained in root's user keyring after this
        // control exits. Never inspect or print unrelated key payloads.
        let seeded = Command::new("/context/probe-runtime/loader")
            .args([
                "--library-path",
                "/context/probe-runtime/lib",
                "/context/probe-runtime/keyctl",
                "add",
                "user",
                "conglobate-validation-sentinel",
                "owned-fixture",
                "@u",
            ])
            .env_clear()
            .env("LC_ALL", "C")
            .output()?;
        assert!(seeded.status.success(), "fixture key creation failed");
        Self::key_control()?;
        println!("COMMAND_KEY_SENTINEL_SEEDED_PASS");
        println!("COMMAND_SYSCALL_CONTROL_PASS");
        Ok(())
    }
    fn key_control() -> Result<(), Box<dyn std::error::Error>> {
        assert!(std::path::Path::new("/conglobate-disposable-validation-vm").is_file());
        assert!(
            fs::read_to_string("/proc/keys")?
                .lines()
                .any(|line| line.contains("conglobate-validation-sentinel")),
            "supervisor fixture key must remain visible"
        );
        Ok(())
    }
    fn proc_visibility() -> Result<(), Box<dyn std::error::Error>> {
        // Only inspect the disposable VM's interfaces. Never print addresses,
        // key descriptions or scheduler contents into the evidence stream.
        let stat = fs::read_to_string("/proc/self/stat")?;
        let pid: u32 = stat
            .split_whitespace()
            .next()
            .ok_or("missing proc PID")?
            .parse()?;
        assert_eq!(pid, std::process::id());
        assert!(!fs::read_to_string("/proc/self/mountinfo")?.is_empty());
        match fs::read_to_string("/proc/kallsyms") {
            Ok(symbols) => {
                assert!(!symbols.is_empty(), "expected kernel symbol control");
                for line in symbols.lines() {
                    let address = line.split_whitespace().next().ok_or("missing address")?;
                    assert!(
                        address.bytes().all(|byte| byte == b'0'),
                        "kernel address visible"
                    );
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error.into()),
        }
        match fs::read_to_string("/proc/keys") {
            Ok(keys) => assert!(keys.trim().is_empty(), "key records visible"),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error.into()),
        }
        match fs::File::open("/proc/sched_debug") {
            Ok(_) => return Err("global scheduler interface visible".into()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error.into()),
        }
        println!("COMMAND_PROC_VISIBILITY_PASS");
        Ok(())
    }
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if std::env::args().nth(1).as_deref() == Some("control") {
            return Self::control();
        }
        if std::env::args().nth(1).as_deref() == Some("key-control-after") {
            Self::key_control()?;
            println!("COMMAND_KEY_SENTINEL_RETAINED_PASS");
            return Ok(());
        }
        let map = fs::read_to_string("/proc/self/uid_map")?;
        let fields: Vec<_> = map.split_whitespace().collect();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], "0");
        assert_ne!(fields[1], "0", "requires mapped command root");
        assert!(fs::read_to_string("/proc/self/status")?.contains("Seccomp:\t2"));
        // Empty unshare flags request no namespace mutation; without seccomp
        // this is a no-op. Zero-length self read needs no foreign-process access.
        assert_eq!(
            nix::sched::unshare(nix::sched::CloneFlags::empty()),
            Err(nix::errno::Errno::EPERM)
        );
        assert_eq!(
            nix::sys::uio::process_vm_readv(nix::unistd::getpid(), &mut [], &[]),
            Err(nix::errno::Errno::EPERM)
        );
        assert_eq!(
            rustix::net::socket_with(
                rustix::net::AddressFamily::NETLINK,
                rustix::net::SocketType::RAW,
                rustix::net::SocketFlags::CLOEXEC,
                None
            )
            .unwrap_err(),
            rustix::io::Errno::PERM
        );
        let keys = Command::new("/probe-runtime/loader")
            .args([
                "--library-path",
                "/probe-runtime/lib",
                "/probe-runtime/keyctl",
                "show",
            ])
            .env_clear()
            .env("LC_ALL", "C")
            .output()?;
        assert!(!keys.status.success());
        let error = String::from_utf8(keys.stderr)?;
        assert!(error.contains("Operation not permitted"), "{error}");
        println!("COMMAND_SYSCALL_DENIAL_PASS");
        Self::proc_visibility()?;
        let (mut sender, mut receiver) = UnixStream::pair()?;
        use io::{Read, Write};
        sender.write_all(b"ok")?;
        let mut data = [0; 2];
        receiver.read_exact(&mut data)?;
        assert_eq!(&data, b"ok");
        assert!(
            Command::new("/bin/sh")
                .args(["-eu", "-c", "test owned = owned"])
                .status()?
                .success()
        );
        println!("COMMAND_ALLOWED_PROCESS_AND_SOCKET_PASS");
        Ok(())
    }
}
fn main() {
    if let Err(error) = CommandProbe::run() {
        eprintln!("command probe: {error}");
        std::process::exit(1);
    }
}
