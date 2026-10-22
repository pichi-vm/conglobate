// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use conglobate::{
    anchor::RecoveryDirectory,
    domain::RecordedDomain,
    filesystem::{Detached, Mount, MountPoint},
    isolation::PrivateMountNamespace,
    process::{ChildReaper, CommandPrivileges, CommandSyscalls, Supervision},
};
use std::{
    fs,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Integration;
impl Integration {
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("/conglobate-disposable-validation-vm").exists() {
            return Err("disposable guest required".into());
        }
        let args: Vec<_> = std::env::args().collect();
        if args
            .get(1)
            .is_some_and(|a| a == "privileges" || a == "syscalls")
        {
            let policy = CommandPrivileges::filesystem();
            let expected = format!("{:016x}", policy.allowed().bits());
            let syscalls = CommandSyscalls::compile()?;
            policy.apply()?;
            if args[1] == "syscalls" {
                syscalls.apply()?;
                // Benign owned-resource checks only. libc thread creation must
                // fall back from clone3 while namespace creation is denied.
                assert_eq!(std::thread::spawn(|| 42).join().unwrap(), 42);
                fs::write("/work/filtered-write", "ordinary-io")?;
                assert_eq!(fs::read_to_string("/work/filtered-write")?, "ordinary-io");
                fs::remove_file("/work/filtered-write")?;
                let blocked = Command::new("/usr/bin/unshare")
                    .args(["--user", "/usr/bin/true"])
                    .env_clear()
                    .output()?;
                assert!(!blocked.status.success());
                assert!(
                    String::from_utf8_lossy(&blocked.stderr).contains("Operation not permitted")
                );
                println!("NATIVE_SYSCALL_NAMESPACE_DENIAL_PASS");
            }
            let script = format!(
                r#"
                caps=0
                nnp=0
                while read -r key value rest; do
                    case "$key" in
                        CapEff:|CapPrm:|CapBnd:) test "$value" = {expected}; caps=$((caps+1));;
                        CapInh:|CapAmb:) test "$value" = 0000000000000000; caps=$((caps+1));;
                        NoNewPrivs:) test "$value" = 1; nnp=1;;
                    esac
                done < /proc/self/status
                test "$caps" = 5
                test "$nnp" = 1
                echo NATIVE_PRIVILEGES_EXEC_PASS
            "#
            );
            return Err(Command::new("/usr/bin/bash")
                .args(["-eu", "-c", &script])
                .env_clear()
                .stdin(Stdio::null())
                .exec()
                .into());
        }
        let mode = args.get(2).ok_or("mode required")?;
        if args.get(1).is_some_and(|a| a == "helper") {
            return Self::helper(mode);
        }
        let reaper = ChildReaper::install()?;
        let recovery = Path::new("/work/integration-records");
        fs::create_dir(recovery)?;
        fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(recovery)?;
        let parent = Path::new("/work/cgroups");
        let domain = RecordedDomain::create(&lease, parent)?;
        let name = domain.record().name().to_owned();
        let mut command = Command::new(std::env::current_exe()?);
        command.args(["helper", mode]).env_clear();
        let helper = domain
            .launch(command, Instant::now() + Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        let ready = Path::new("/work/origin/command-started");
        let setup = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            if helper.supervision().peer_lost()? {
                return Err("helper exited before command write".into());
            }
            if Instant::now() >= setup {
                return Err("command start deadline".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if mode == "vm-death" {
            fs::write("/work/supervisor-ready", "running\n")?;
            // The independent guest controller terminates this process. There
            // must be no successful step receipt or in-process cleanup path.
            helper
                .supervision()
                .wait_for_loss_until(Instant::now() + Duration::from_secs(30))?;
            return Err("command ended before supervisor-death exercise".into());
        }
        let execution =
            Instant::now() + Duration::from_millis(if mode == "timeout" { 200 } else { 5000 });
        let observation = helper.supervision().wait_for_loss_until(execution);
        if mode == "timeout" {
            assert_eq!(
                observation.unwrap_err().kind(),
                std::io::ErrorKind::TimedOut
            );
        } else {
            observation?;
        }
        let teardown = Instant::now() + Duration::from_secs(5);
        let (status, _) = helper
            .cancel_until(teardown)
            .map_err(|e| e.to_string())?
            .reap_until(teardown)
            .map_err(|e| e.to_string())?
            .with_reaper(reaper)
            .drain_until(teardown)
            .map_err(|e| e.to_string())?
            .finish(parent)
            .map_err(|e| e.to_string())?;
        assert_eq!(status.success(), mode == "success");
        assert!(!parent.join(name).exists());
        assert_eq!(fs::metadata(ready)?.uid(), 0);
        assert_eq!(fs::read_to_string(ready)?, "image-write\n");
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(fs::read_to_string("/supervisor-secret")?, "unchanged");
        drop(lease);
        fs::remove_dir(recovery)?;
        println!("INTEGRATED_COMMAND_{}_PASS", mode.to_uppercase());
        Ok(())
    }
    fn helper(mode: &str) -> Result<(), Box<dyn std::error::Error>> {
        let channel = Supervision::from_stdin()?
            .exchange_until(Instant::now() + Duration::from_secs(5))?
            .join_domain_until(Instant::now() + Duration::from_secs(5))?;
        let namespace = PrivateMountNamespace::enter()?;
        let holder = fs::read_to_string("/work/userns-pid")?;
        let userns = format!("/proc/{}/ns/user", holder.trim());
        let mount = Mount::<Detached>::clone_from(&fs::File::open("/work/origin")?, &namespace)?
            .idmap(fs::File::open(&userns)?)
            .map_err(|e| e.to_string())?
            .attach(MountPoint::create(Path::new("/work"))?)
            .map_err(|e| e.to_string())?;
        // Fixture adapter only: production must pass a scoped root handle, not
        // discover generated mount paths. There is exactly one attachment here.
        let targets: Vec<_> = fs::read_dir("/work")?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|e| {
                e.file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-mount-")
            })
            .collect();
        assert_eq!(targets.len(), 1);
        let root = targets[0].path();
        let checks = r#"
            test ! -e /supervisor-secret
            test ! -e /work
            test ! -e /idmap-probe
            test ! -e /image-root
            caps=0
            nnp=0
            while read -r key value rest; do
                case "$key" in
                    CapInh:|CapPrm:|CapEff:|CapBnd:|CapAmb:)
                        test "$value" = 0000000000000000
                        caps=$((caps + 1));;
                    NoNewPrivs:) test "$value" = 1; nnp=1;;
                esac
            done < /proc/self/status
            test "$caps" = 5
            test "$nnp" = 1
            read -r inside outside count < /proc/self/uid_map
            test "$inside" = 0
            test "$outside" = 100000
            test "$count" = 65536
            printf 'privileges-checked\n' > /command-privileges
            printf 'image-write\n' > /command-started
        "#;
        let script = format!(
            "{checks}\n{}",
            if mode == "timeout" || mode == "vm-death" {
                "while :; do :; done"
            } else {
                "printf 'completed\\n' > /command-completed"
            }
        );
        let status = Command::new("/usr/bin/nsenter")
            .arg(format!("--user={userns}"))
            .args([
                "/usr/bin/bwrap",
                "--unshare-pid",
                "--unshare-net",
                "--unshare-ipc",
                "--unshare-uts",
                "--new-session",
                "--cap-drop",
                "ALL",
                "--cap-add",
                "CAP_SETPCAP",
                "--bind",
            ])
            .arg(root)
            .arg("/")
            .args([
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--ro-bind",
                "/empty-sys",
                "/sys",
                "--clearenv",
                "--setenv",
                "PATH",
                "/bin",
                "--setenv",
                "HOME",
                "/",
                "--chdir",
                "/",
                "/bin/setpriv",
                "--bounding-set=-all",
                "--inh-caps=-all",
                "--ambient-caps=-all",
                "--no-new-privs",
                "/bin/sh",
                "-eu",
                "-c",
                &script,
            ])
            .stdin(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(format!("command failed: {status}").into());
        }
        mount.close().map_err(|e| e.to_string())?;
        namespace.restore()?;
        drop(channel);
        Ok(())
    }
}
fn main() {
    if let Err(error) = Integration::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
