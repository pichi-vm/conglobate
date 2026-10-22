// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
use conglobate::{
    anchor::RecoveryDirectory,
    domain::RecordedDomain,
    filesystem::{Detached, Mount, MountPoint},
    isolation::{
        CommandDevices, CommandNamespaces, CommandRoot, IdRange, MappedUserNamespace,
        PrivateMountNamespace,
    },
    policy::{FilesystemTarget, LabelPolicy, LabelScope, PinDirectory, PinnedDisabled},
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
        if args.get(1).is_some_and(|a| a == "namespace-seed") {
            MappedUserNamespace::serve(Instant::now() + Duration::from_secs(5))?;
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "namespace-seed-unchanged") {
            let deadline = Instant::now() + Duration::from_secs(5);
            Supervision::from_stdin()?
                .exchange_until(deadline)?
                .wait_for_loss_until(deadline)?;
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "namespace-rejections") {
            let error =
                CommandNamespaces::enter(fs::File::open("/supervisor-secret")?).unwrap_err();
            assert!(error.to_string().contains("requires nsfs"));
            let userns = fs::File::open("/proc/self/ns/user")?;
            let (release, gate) = std::sync::mpsc::channel::<()>();
            let (ready, entered) = std::sync::mpsc::channel::<()>();
            let thread = std::thread::spawn(move || {
                ready.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            });
            entered.recv_timeout(Duration::from_secs(5))?;
            let error = CommandNamespaces::enter(userns).unwrap_err();
            release.send(())?;
            thread.join().unwrap();
            assert!(error.to_string().contains("requires one thread"));
            let before = fs::read_to_string("/proc/self/uid_map")?;
            let mut creator = Command::new(std::env::current_exe()?);
            creator.arg("namespace-seed-unchanged").env_clear();
            let deadline = Instant::now() + Duration::from_secs(5);
            let failure = MappedUserNamespace::create(
                creator,
                IdRange::new(100000, 65536)?,
                IdRange::new(100000, 65536)?,
                deadline,
            )
            .unwrap_err();
            match failure {
                conglobate::isolation::UserNamespaceFailure::Mapping { source, process } => {
                    assert!(
                        source
                            .to_string()
                            .contains("did not create a new user namespace")
                    );
                    assert!(process.wait_until(deadline)?.status().success());
                }
                failure => return Err(failure.into()),
            }
            assert_eq!(fs::read_to_string("/proc/self/uid_map")?, before);
            assert!(
                fs::read_to_string("/proc/thread-self/children")?
                    .trim()
                    .is_empty()
            );
            println!("NATIVE_NAMESPACE_SETUP_REJECTIONS_PASS");
            return Ok(());
        }
        if args.get(1).is_some_and(|a| a == "native-namespaces") {
            let userns = args.get(2).ok_or("user namespace required")?;
            let root = args.get(3).ok_or("root required")?;
            let script = args.get(4).ok_or("script required")?;
            let userns = fs::File::open(userns)?;
            let expected_user = userns.metadata()?.ino();
            let names = ["mnt", "net", "ipc", "uts", "pid", "pid_for_children"];
            let before =
                names.map(|name| fs::metadata(format!("/proc/self/ns/{name}")).unwrap().ino());
            let scope = CommandNamespaces::enter(userns)?;
            for (name, old) in names.into_iter().zip(before) {
                let new = fs::metadata(format!("/proc/self/ns/{name}"))?.ino();
                if name == "pid" {
                    assert_eq!(new, old);
                } else {
                    assert_ne!(new, old);
                }
            }
            assert_eq!(fs::metadata("/proc/self/ns/user")?.ino(), expected_user);
            let status = fs::read_to_string("/proc/self/status")?;
            for line in status.lines() {
                let mut values = line.split_whitespace();
                match values.next() {
                    Some("Uid:" | "Gid:") => assert_eq!(values.collect::<Vec<_>>(), ["0"; 4]),
                    Some("Groups:") => assert_eq!(values.count(), 0),
                    _ => {}
                }
            }
            for line in fs::read_to_string("/proc/self/mountinfo")?.lines() {
                let optional = line
                    .split_whitespace()
                    .skip(6)
                    .take_while(|field| *field != "-");
                assert!(
                    !optional
                        .into_iter()
                        .any(|field| field.starts_with("shared:") || field.starts_with("master:"))
                );
            }
            println!("NATIVE_NAMESPACE_IDENTITIES_AND_CREDENTIALS_PASS");
            let mut setup = Command::new(std::env::current_exe()?);
            setup.args(["native-root", root, script]).env_clear();
            // Keep the private device-transfer socket on stdin for root entry.
            let child = scope.spawn(setup)?;
            let status = child
                .wait_until(Instant::now() + Duration::from_secs(30))?
                .status();
            use std::os::unix::process::ExitStatusExt;
            std::process::exit(status.code().unwrap_or(128 + status.signal().unwrap_or(1)));
        }
        if args.get(1).is_some_and(|a| a == "native-root") {
            let root = Path::new(args.get(2).ok_or("root required")?);
            let script = args.get(3).ok_or("script required")?;
            let scope = PrivateMountNamespace::enter()?;
            let selected = CommandRoot::from_directory(
                fs::File::open(root)?,
                scope,
                CommandDevices::from_input()?,
            )?;
            let filter = CommandSyscalls::compile()?;
            let entered = selected.enter()?;
            entered.execute(script, CommandPrivileges::filesystem(), filter)?;
            unreachable!();
        }
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
        // Fixture init retains the original filesystem mount. The native
        // namespace pin below retains its identity until policy cleanup/poweroff.
        for name in ["orphan-release", "survivor-ready"] {
            match fs::remove_file(Path::new("/work/origin").join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let reaper = ChildReaper::install()?;
        let recovery = Path::new("/work/integration-records");
        fs::create_dir(recovery)?;
        fs::set_permissions(recovery, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(recovery)?;
        let mut creator = Command::new(std::env::current_exe()?);
        creator.arg("namespace-seed").env_clear();
        let namespace = MappedUserNamespace::create(
            creator,
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
            Instant::now() + Duration::from_secs(5),
        )?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        let namespace = namespace.pin(&lease)?;
        fs::write(
            "/work/userns-path",
            recovery
                .join(namespace.record().name())
                .as_os_str()
                .as_encoded_bytes(),
        )?;
        let policy = EnabledPolicy::install(mode != "policy-off")?;
        println!("NATIVE_MAPPED_NAMESPACE_CREATOR_REAPED_PASS");

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
            // The fixture helper has closed its channel but cannot exit until
            // explicitly released. Channel loss must not count as process exit.
            if mode != "shell-failure" {
                assert_eq!(
                    helper
                        .supervision()
                        .wait_for_peer_exit_until(Instant::now())
                        .unwrap_err()
                        .kind(),
                    std::io::ErrorKind::TimedOut,
                );
                ExitGate::release()?;
            }
            helper.supervision().wait_for_peer_exit_until(execution)?;
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
        // This is the only policy teardown site: cancellation, helper reaping
        // and descendant draining have all completed above.
        policy.disable()?;
        namespace.release()?;
        assert_eq!(status.success(), mode == "success" || mode == "policy-off");
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
        let userns = fs::read_to_string("/work/userns-path")?;
        let mount = Mount::<Detached>::clone_from(&fs::File::open("/work/origin")?, &namespace)?
            .idmap(fs::File::open(&userns)?)
            .map_err(|e| e.to_string())?
            .attach(MountPoint::create(Path::new("/work"))?)
            .map_err(|e| e.to_string())?;
        // Fixture adapter only: production must pass a scoped root handle, not
        // discover generated mount paths. Match the actual mount identity so
        // an empty target left by a cancelled prior helper cannot be selected.
        let expected = mount.root_metadata()?;
        let targets: Vec<_> = fs::read_dir("/work")?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|e| {
                e.file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-mount-")
                    && e.metadata().is_ok_and(|metadata| {
                        (metadata.dev(), metadata.ino()) == (expected.dev(), expected.ino())
                    })
            })
            .collect();
        assert_eq!(targets.len(), 1);
        let root = targets[0].path();
        let checks = r#"
            test "$PPID" = 1
            test "$$" -ne 1
            for interface in exe mem maps environ fd/0; do
                if (: < "/proc/1/$interface") 2>/dev/null; then exit 1; fi
            done
            # The intermediate shell exits, leaving a live orphan that waits
            # for a file-based release. No timing delay determines adoption.
            /bin/sh -c '/bin/sh -c "while test ! -e /orphan-release; do :; done" & echo $! > /orphan-id'
            read -r orphan < /orphan-id
            adopted=0
            while read -r key value rest; do
                if test "$key" = PPid:; then test "$value" = 1; adopted=1; fi
            done < "/proc/$orphan/status"
            test "$adopted" = 1
            printf 'exit' > /orphan-release
            while test -e "/proc/$orphan"; do :; done
            echo NATIVE_NAMESPACE_INIT_ADOPTION_PASS
            test ! -e /supervisor-secret
            test ! -e /work
            test ! -e /idmap-probe
            test ! -e /image-root
            test ! -e /bin/setpriv
            for node in null zero full random urandom; do
                test -c "/dev/$node"
                test -O "/dev/$node"
            done
            printf 'discarded' > /dev/null
            if (printf 'full' > /dev/full) 2>/dev/null; then exit 1; fi
            test ! -e /dev/fuse
            set -- /dev/*
            test "$#" = 13
            test -c /dev/tty
            if (: > /dev/tty) 2>/dev/null; then exit 1; fi
            test -c /dev/ptmx
            test ! -e /dev/pts/0
            exec 3<> /dev/ptmx
            test -c /dev/pts/0
            exec 3>&-
            test ! -e /dev/pts/0
            test ! -e /dev/shm/step-data
            printf 'temporary' > /dev/shm/step-data
            read -r -N 9 shm < /dev/shm/step-data
            test "$shm" = temporary
            if (printf no > /sys/must-be-read-only) 2>/dev/null; then exit 1; fi
            restricted=0
            while read -r id parent device root point options rest; do
                case "$point" in
                    /proc/sys|/proc/sysrq-trigger|/proc/irq|/proc/bus)
                        case ",$options," in
                            *,ro,*) restricted=$((restricted + 1));;
                            *) exit 1;;
                        esac;;
                esac
            done < /proc/self/mountinfo
            test "$restricted" = 4
            # Open-only denial: do not send a command to any kernel control.
            if (: > /proc/sysrq-trigger) 2>/dev/null; then exit 1; fi
            caps=0
            nnp=0
            while read -r key value rest; do
                case "$key" in
                    CapPrm:|CapEff:|CapBnd:)
                        test "$value" = EXPECTED_CAPABILITIES
                        caps=$((caps + 1));;
                    CapInh:|CapAmb:)
                        test "$value" = 0000000000000000
                        caps=$((caps + 1));;
                    NoNewPrivs:) test "$value" = 1; nnp=1;;
                esac
            done < /proc/self/status
            test "$caps" = 5
            test "$nnp" = 1
            read -r groups_policy < /proc/self/setgroups
            test "$groups_policy" = allow
            read -r inside outside count < /proc/self/uid_map
            test "$inside" = 0
            test "$outside" = 100000
            test "$count" = 65536
            printf 'privileges-checked\n' > /command-privileges
            printf 'image-write\n' > /command-started
        "#;
        let checks = checks.replace(
            "EXPECTED_CAPABILITIES",
            &format!("{:016x}", CommandPrivileges::filesystem().allowed().bits()),
        );
        let labels = if mode == "policy-off" {
            r#"
                printf 'owned' > /label-policy-off
                if setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /label-policy-off; then exit 1; fi
            "#
        } else {
            r#"
                printf 'owned' > /labelled
                setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /labelled
                test "$(getfattr --only-values -n security.selinux /labelled)" = 'system_u:object_r:etc_t:s0'
                setfattr -x security.selinux /labelled
                if getfattr --only-values -n security.selinux /labelled; then exit 1; fi
                setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /labelled
                printf 'owned' > /dev/shm/outside-policy
                if setfattr -n security.selinux -v 'system_u:object_r:etc_t:s0' /dev/shm/outside-policy; then exit 1; fi
                if setfattr -n security.selinux.extra -v 'test' /labelled; then exit 1; fi
            "#
        };
        let script = format!(
            "{labels}\n{checks}\n{}",
            if mode == "vm-death" {
                r#"
                    while test ! -e /supervisor-gone; do :; done
                    setfattr -n security.selinux -v 'system_u:object_r:var_t:s0' /labelled
                    test "$(getfattr --only-values -n security.selinux /labelled)" = 'system_u:object_r:var_t:s0'
                    if setfattr -n security.selinux -v 'system_u:object_r:var_t:s0' /dev/shm/outside-policy; then exit 1; fi
                    printf 'checked' > /death-policy-checked
                    while :; do :; done
                "#
            } else if mode == "timeout" {
                "while :; do :; done"
            } else if mode == "shell-failure" {
                "exit 37"
            } else {
                r#"
                    /bin/sh -c "printf 'running' > /survivor-ready; while :; do :; done" &
                    while test ! -e /survivor-ready; do :; done
                    printf 'completed\n' > /command-completed
                "#
            }
        );
        let status = Command::new(std::env::current_exe()?)
            .arg("native-namespaces")
            .arg(&userns)
            .arg(root)
            .arg(&script)
            .stdin(CommandDevices::create(100000, 100000)?.into_input()?)
            .status()?;
        if mode == "shell-failure" {
            assert_eq!(status.code(), Some(37));
            println!("NATIVE_NAMESPACE_INIT_EXIT_STATUS_PASS");
        }
        if !status.success() {
            return Err(format!("command failed: {status}").into());
        }
        mount.close().map_err(|e| e.to_string())?;
        namespace.restore()?;
        let exit = ExitGate::bind()?;
        drop(channel);
        exit.await_release()?;
        Ok(())
    }
}
/// Fixture-only handshake keeps the helper alive after channel closure without
/// relying on scheduler timing. Commands cannot access this supervisor path.
#[derive(Debug)]
struct ExitGate(std::os::unix::net::UnixDatagram);
impl ExitGate {
    const PATH: &str = "/work/helper-exit-release";

    fn bind() -> std::io::Result<Self> {
        let socket = std::os::unix::net::UnixDatagram::bind(Self::PATH)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self(socket))
    }
    fn release() -> std::io::Result<()> {
        let socket = std::os::unix::net::UnixDatagram::unbound()?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        assert_eq!(socket.send_to(b"exit", Self::PATH)?, 4);
        Ok(())
    }
    fn await_release(self) -> std::io::Result<()> {
        let mut message = [0; 5];
        let count = self.0.recv(&mut message)?;
        assert_eq!(&message[..count], b"exit");
        fs::remove_file(Self::PATH)?;
        Ok(())
    }
}

/// Validation-only activation adapter. Production exposes disabled policy only.
/// Leak-on-error deliberately retains every pin; init owns the bpffs mount until
/// VM disposal. Never reuse this adapter as a production admission capability.
#[derive(Debug)]
struct EnabledPolicy {
    policy: std::mem::ManuallyDrop<LabelPolicy<PinnedDisabled>>,
    config: libbpf_rs::MapHandle,
}
impl EnabledPolicy {
    fn install(enable: bool) -> Result<Self, Box<dyn std::error::Error>> {
        use libbpf_rs::{MapCore, MapFlags, MapHandle};
        let namespace = fs::metadata(fs::read_to_string("/work/userns-path")?)?.ino();
        let target = FilesystemTarget::new(
            fs::metadata("/proc/self/ns/user")?.ino(),
            fs::metadata("/work/origin")?.dev(),
        )?;
        let scope = LabelScope::new(namespace, 100000, [target])?;
        let subject = scope.subject_value();
        let pinned = LabelPolicy::new(scope)?
            .load()?
            .pin_disabled(PinDirectory::create(Path::new("/work/pins"))?)?;
        let paths = fs::read_dir("/work/pins")?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(paths.len(), 1);
        let config = MapHandle::from_pinned_path(paths[0].join("config"))?;
        let owner = Self {
            policy: std::mem::ManuallyDrop::new(pinned),
            config,
        };
        if enable {
            owner
                .config
                .update(&0u32.to_ne_bytes(), &subject, MapFlags::ANY)?;
        }
        Ok(owner)
    }
    fn disable(self) -> Result<(), Box<dyn std::error::Error>> {
        use libbpf_rs::{MapCore, MapFlags};
        // Caller has completed cancellation and process-wide descendant drain.
        // Failure retains pinned policy through VM disposal, never drops guards.
        self.config
            .update(&0u32.to_ne_bytes(), &[0; 8], MapFlags::ANY)?;
        std::mem::ManuallyDrop::into_inner(self.policy).close()?;
        assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
        Ok(())
    }
}

fn main() {
    if let Err(error) = Integration::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
