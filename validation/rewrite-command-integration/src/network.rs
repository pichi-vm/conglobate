// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::{
        CommandDevices, CommandSetup, ExecutionGate, GatedCommandSetup, IdRange, Mapped,
        UserNamespace,
    },
    network::{
        FirewallTable, InterfaceName, NatPolicy, NetworkAddress, NetworkNamespace, VethPair,
    },
    process::Process,
    step::SetupBudget,
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    time::{Duration, Instant},
};

pub(crate) struct NetworkProbe;
impl NetworkProbe {
    fn resolver_mount() -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            isolation::PrivateMountNamespace,
            network::{ResolverInput, ResolverSource, ResolverTarget},
            storage::Workspace,
        };
        use std::{fs::File, os::unix::fs::symlink};
        let scope = PrivateMountNamespace::enter()?;
        let workspace = Workspace::create()?;
        let fixture = Path::new("/work/resolver-probe");
        fs::create_dir(fixture)?;
        let source = fixture.join("source");
        let bytes = b"nameserver 192.0.2.53\nsearch test.invalid\n# exact \xff\n";
        fs::write(&source, bytes)?;
        let input = ResolverInput::capture(&source)?;
        fs::remove_file(source)?;
        let source = ResolverSource::prepare(&input, &workspace)?;
        for existing in [false, true] {
            let image = fixture.join(if existing { "existing" } else { "missing" });
            fs::create_dir_all(image.join("etc"))?;
            symlink("/run/resolver", image.join("etc/resolv.conf"))?;
            if existing {
                fs::create_dir(image.join("run"))?;
                fs::write(image.join("run/resolver"), b"original")?;
            }
            eprintln!("resolver selecting target existing={existing}");
            let target = ResolverTarget::resolve(File::open(&image)?)?;
            let mountpoint = image.join(target.path());
            let wrong_namespace = PrivateMountNamespace::enter()?;
            assert!(
                source
                    .mount()?
                    .attach(&target)
                    .unwrap_err()
                    .to_string()
                    .contains("another mount namespace")
            );
            wrong_namespace.restore()?;
            let mount = source.mount()?;
            eprintln!("resolver mount prepared");
            mount.attach(&target)?;
            eprintln!("resolver mount attached");
            assert_eq!(fs::read(&mountpoint)?, bytes);
            let error = fs::OpenOptions::new()
                .write(true)
                .open(&mountpoint)
                .unwrap_err();
            assert_eq!(
                error.raw_os_error(),
                Some(rustix::io::Errno::ROFS.raw_os_error())
            );
            rustix::mount::unmount(&mountpoint, rustix::mount::UnmountFlags::empty())?;
            target.release()?;
            assert_eq!(
                fs::read_link(image.join("etc/resolv.conf"))?,
                Path::new("/run/resolver")
            );
            if existing {
                assert_eq!(fs::read(image.join("run/resolver"))?, b"original");
            } else {
                assert!(!image.join("run").exists());
            }
        }
        source.close()?;
        fs::remove_dir_all(fixture)?;
        drop(workspace);
        scope.restore()?;
        println!("NETWORK_RESOLVER_READONLY_EXACT_BYTES_AND_RESTORATION_PASS");
        Ok(())
    }
    pub(crate) fn entry(
        gated: bool,
        expected: u64,
        traffic: bool,
        deny: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if gated {
            let (_scope, _root) = GatedCommandSetup::from_input()?.enter()?;
        } else {
            let (_scope, _devices) = CommandSetup::from_input()?.enter()?;
        }
        assert_eq!(fs::metadata("/proc/self/ns/net")?.ino(), expected);
        assert_eq!(rustix::process::getuid().as_raw(), 0);
        assert!(
            fs::read_to_string("/proc/self/uid_map")?
                .split_whitespace()
                .eq(["0", "100000", "65536"])
        );
        for entry in fs::read_dir("/proc/self/fd")? {
            if let Ok(target) = fs::read_link(entry?.path()) {
                assert_ne!(target.to_string_lossy(), format!("net:[{expected}]"));
                assert!(!target.to_string_lossy().contains("resolver-"));
            }
        }
        assert_eq!(fs::read("etc/resolv.conf")?, b"nameserver 192.0.2.53\n");
        let denied = fs::OpenOptions::new()
            .write(true)
            .open("etc/resolv.conf")
            .unwrap_err()
            .raw_os_error();
        assert!(
            denied == Some(rustix::io::Errno::ROFS.raw_os_error())
                || denied == Some(rustix::io::Errno::ACCESS.raw_os_error())
        );
        assert_eq!(fs::read("etc/resolv.conf")?, b"nameserver 192.0.2.53\n");
        if traffic {
            for (bind, remote, gateway) in [
                ("0.0.0.0:0", "198.51.100.42:7", "10.44.0.1:45454"),
                ("[::]:0", "[2001:db8::42]:7", "[fd42:44::1]:45454"),
            ] {
                let socket = std::net::UdpSocket::bind(bind)?;
                socket.connect(remote)?;
                assert!(!socket.local_addr()?.ip().is_unspecified());
                let unsolicited = std::net::UdpSocket::bind(if bind.starts_with('[') {
                    "[::]:45455"
                } else {
                    "0.0.0.0:45455"
                })?;
                unsolicited.set_read_timeout(Some(Duration::from_secs(1)))?;
                socket.set_read_timeout(Some(Duration::from_secs(3)))?;
                socket.connect(gateway)?;
                socket.send(b"private-network")?;
                let mut bytes = [0; 32];
                let count = socket.recv(&mut bytes)?;
                assert_eq!(&bytes[..count], b"reply");
                let incoming = unsolicited.recv(&mut bytes);
                if deny {
                    let error = incoming.unwrap_err();
                    assert!(matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ));
                } else {
                    assert_eq!(&bytes[..incoming?], b"unsolicited");
                }
            }
            for bind in ["127.0.0.1:0", "[::1]:0"] {
                let socket = std::net::UdpSocket::bind(bind)?;
                socket.set_read_timeout(Some(Duration::from_secs(3)))?;
                socket.send_to(b"loopback", socket.local_addr()?)?;
                let mut bytes = [0; 32];
                let count = socket.recv(&mut bytes)?;
                assert_eq!(&bytes[..count], b"loopback");
            }
        }
        Ok(())
    }
    pub(crate) fn offline(gated: bool, original: u64) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
        if gated {
            let (_scope, _root) = GatedCommandSetup::from_input()?.enter_until(deadline)?;
        } else {
            let (_scope, _devices) = CommandSetup::from_input()?.enter_until(deadline)?;
        }
        assert_ne!(fs::metadata("/proc/self/ns/net")?.ino(), original);
        let devices = fs::read_to_string("/proc/net/dev")?;
        let names: Vec<_> = devices
            .lines()
            .skip(2)
            .map(|line| line.split(':').next().unwrap().trim())
            .collect();
        assert_eq!(names, ["lo"]);
        for bind in ["127.0.0.1:0", "[::1]:0"] {
            let socket = std::net::UdpSocket::bind(bind)?;
            socket.set_read_timeout(Some(Duration::from_secs(3)))?;
            socket.send_to(b"offline-loopback", socket.local_addr()?)?;
            let mut bytes = [0; 32];
            let count = socket.recv(&mut bytes)?;
            assert_eq!(&bytes[..count], b"offline-loopback");
        }
        for (bind, remote) in [
            ("0.0.0.0:0", "198.51.100.42:7"),
            ("[::]:0", "[2001:db8::42]:7"),
        ] {
            let socket = std::net::UdpSocket::bind(bind)?;
            assert!(socket.connect(remote).is_err());
        }
        Ok(())
    }
    fn entry_roundtrip(
        program: &HelperProgram,
        network: &NetworkNamespace,
        traffic: bool,
        deny: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let parent = Path::new("/work/network-user-pin");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let recovery = RecoveryDirectory::open(parent)?;
        let cutoff = SetupBudget::begin().cutoff()?;
        let user = UserNamespace::<Mapped>::create(
            program.namespace_creator(cutoff)?,
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
            cutoff.deadline()?,
        )?
        .pin(&recovery)?;
        let listeners = if traffic {
            ["10.44.0.1:45454", "[fd42:44::1]:45454"]
                .into_iter()
                .map(std::net::UdpSocket::bind)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        for gated in [false, true] {
            use conglobate::{
                network::{ResolverInput, ResolverSource, ResolverTarget},
                storage::Workspace,
            };
            let image = Path::new("/work/network-resolver-root");
            fs::create_dir(image)?;
            let target = ResolverTarget::resolve(fs::File::open(image)?)?;
            let source_path = Path::new("/work/network-resolver-input");
            fs::write(source_path, b"nameserver 192.0.2.53\n")?;
            let input = ResolverInput::capture(source_path)?;
            fs::remove_file(source_path)?;
            let workspace = Workspace::create()?;
            let source = ResolverSource::prepare(&input, &workspace)?;
            let setup = CommandSetup::new(
                fs::File::open(parent.join(user.record().name()))?,
                CommandDevices::create(100000, 100000)?,
            )?
            .with_network(network)?
            .with_resolver(&source)?;
            let (gate, endpoint) = ExecutionGate::pair()?;
            let input = if gated {
                setup.with_gate(endpoint).into_input()?
            } else {
                setup.into_input()?
            };
            let child = std::process::Command::new(std::env::current_exe()?)
                .arg("network-entry-probe")
                .arg(gated.to_string())
                .arg(network.metadata()?.ino().to_string())
                .arg(traffic.to_string())
                .arg(deny.to_string())
                .current_dir(image)
                .stdin(input)
                .spawn()?;
            for listener in &listeners {
                listener.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut bytes = [0; 32];
                let (count, peer) = listener.recv_from(&mut bytes)?;
                assert_eq!(&bytes[..count], b"private-network");
                listener.send_to(b"reply", peer)?;
                let target = std::net::SocketAddr::new(peer.ip(), 45455);
                let result = listener.send_to(b"unsolicited", target);
                if !deny {
                    result?;
                }
            }
            let status = Process::from(child)
                .wait_until(Instant::now() + Duration::from_secs(10))?
                .status();
            assert!(status.success());
            // Child mount namespaces are gone; the supervisor sees its empty
            // placeholder, never injected bytes, before removing scaffolding.
            assert!(fs::read(image.join("etc/resolv.conf"))?.is_empty());
            target.release()?;
            assert!(!image.join("etc").exists());
            fs::remove_dir(image)?;
            source.close()?;
            drop(gate);
        }
        println!("NETWORK_RESOLVER_SETUP_TRANSPORT_MAPPED_ROOT_AND_CLEANUP_PASS");
        if !traffic {
            for gated in [false, true] {
                let setup = CommandSetup::new(
                    fs::File::open(parent.join(user.record().name()))?,
                    CommandDevices::create(100000, 100000)?,
                )?;
                let (gate, endpoint) = ExecutionGate::pair()?;
                let input = if gated {
                    setup.with_gate(endpoint).into_input()?
                } else {
                    setup.into_input()?
                };
                let child = std::process::Command::new(std::env::current_exe()?)
                    .arg("network-offline-probe")
                    .arg(gated.to_string())
                    .arg(fs::metadata("/proc/self/ns/net")?.ino().to_string())
                    .stdin(input)
                    .spawn()?;
                assert!(
                    Process::from(child)
                        .wait_until(Instant::now() + Duration::from_secs(10))?
                        .status()
                        .success()
                );
                drop(gate);
            }
            println!("NETWORK_OFFLINE_LOOPBACK_WITHOUT_EXTERNAL_ROUTE_PASS");
        }
        user.release()?;
        drop(recovery);
        fs::remove_dir(parent)?;
        println!("NETWORK_COMMAND_ENTRY_AND_DESCRIPTOR_CLOSURE_PASS");
        Ok(())
    }
    pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
        Self::resolver_mount()?;
        let original = fs::metadata("/proc/self/ns/net")?;
        let program = HelperProgram::current()?;
        let first = NetworkNamespace::create(&program, SetupBudget::begin().cutoff()?)?;
        let second = NetworkNamespace::create(&program, SetupBudget::begin().cutoff()?)?;
        assert_ne!(first.metadata()?.ino(), original.ino());
        assert_ne!(second.metadata()?.ino(), original.ino());
        assert_ne!(first.metadata()?.ino(), second.metadata()?.ino());
        assert_eq!(fs::metadata("/proc/self/ns/net")?.ino(), original.ino());
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        Self::entry_roundtrip(&program, &first, false, false)?;
        drop(first);
        drop(second);
        println!("NETWORK_NAMESPACE_CREATOR_REAPED_PASS");
        let parent = Path::new("/work/network-control");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let host = InterfaceName::try_from("cg-host")?;
        let peer = InterfaceName::try_from("cg-peer")?;
        let pair = VethPair::create(
            parent,
            host.clone(),
            peer.clone(),
            Instant::now() + Duration::from_secs(10),
        )?;
        let alias = fs::read_to_string("/sys/class/net/cg-host/address")?;
        assert!(!alias.trim().is_empty());
        assert_ne!(alias, fs::read_to_string("/sys/class/net/cg-peer/address")?);
        for name in ["cg-host", "cg-peer"] {
            let flags = fs::read_to_string(format!("/sys/class/net/{name}/flags"))?;
            assert_eq!(
                u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16)? & 1,
                0
            );
        }
        let collision = VethPair::create(
            parent,
            host.clone(),
            peer.clone(),
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap_err();
        collision.cleanup()?;
        assert_eq!(alias, fs::read_to_string("/sys/class/net/cg-host/address")?);
        assert!(Path::new("/sys/class/net/cg-peer").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 1);
        println!("NETWORK_EXCLUSIVE_COLLISION_PRESERVES_OWNER_PASS");
        pair.close()?;
        assert!(!Path::new("/sys/class/net/cg-host").exists());
        assert!(!Path::new("/sys/class/net/cg-peer").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("NETWORK_PAIR_EXPLICIT_TEARDOWN_PASS");
        VethPair::create(parent, host, peer, Instant::now())
            .unwrap_err()
            .cleanup()?;
        assert!(!Path::new("/sys/class/net/cg-host").exists());
        assert!(!Path::new("/sys/class/net/cg-peer").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        let pair = VethPair::create(
            parent,
            InterfaceName::try_from("cg-host")?,
            InterfaceName::try_from("cg-peer")?,
            Instant::now() + Duration::from_secs(10),
        )?;
        let namespace = NetworkNamespace::create(&program, SetupBudget::begin().cutoff()?)?;
        let connection = pair.connect(namespace, Instant::now() + Duration::from_secs(10))?;
        assert!(Path::new("/sys/class/net/cg-host").exists());
        assert!(!Path::new("/sys/class/net/cg-peer").exists());
        let host = vec![
            NetworkAddress::new("10.44.0.1".parse()?, 30, None)?,
            NetworkAddress::new("fd42:44::1".parse()?, 64, None)?,
        ];
        let peer = vec![
            NetworkAddress::new("10.44.0.2".parse()?, 30, Some("10.44.0.1".parse()?))?,
            NetworkAddress::new("fd42:44::2".parse()?, 64, Some("fd42:44::1".parse()?))?,
        ];
        let configured =
            connection.configure(host, peer, &program, SetupBudget::begin().cutoff()?)?;
        let policy = NatPolicy::install(&configured, Instant::now() + Duration::from_secs(10))?;
        let listed = std::process::Command::new("/usr/bin/nft")
            .args(["list", "table", "inet", "conglobate_cg_host"])
            .output()?;
        assert!(listed.status.success());
        println!("NFT_POLICY {}", String::from_utf8_lossy(&listed.stdout));
        assert!(
            FirewallTable::create(
                "conglobate_cg_host",
                Instant::now() + Duration::from_secs(10)
            )
            .is_err()
        );
        Self::entry_roundtrip(&program, configured.namespace(), true, true)?;
        policy
            .close(Instant::now() + Duration::from_secs(10))
            .expect("acknowledged policy teardown");
        Self::entry_roundtrip(&program, configured.namespace(), true, false)?;
        assert!(
            !std::process::Command::new("/usr/bin/nft")
                .args(["list", "table", "inet", "conglobate_cg_host"])
                .output()?
                .status
                .success()
        );
        println!("NETWORK_NFT_REPLY_ONLY_AND_TEARDOWN_PASS");
        configured.close()?;
        println!("NETWORK_DUAL_STACK_ADDRESSES_ROUTES_LOOPBACK_PASS");
        assert!(!Path::new("/sys/class/net/cg-host").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("NETWORK_PEER_NAMESPACE_HANDOFF_CLEANUP_PASS");
        let pair = VethPair::create(
            parent,
            InterfaceName::try_from("cg-host")?,
            InterfaceName::try_from("cg-peer")?,
            Instant::now() + Duration::from_secs(10),
        )?;
        let namespace = NetworkNamespace::create(&program, SetupBudget::begin().cutoff()?)?;
        let connection = pair.connect(namespace, Instant::now() + Duration::from_secs(10))?;
        let failure = connection
            .configure(
                vec![NetworkAddress::new("10.44.0.1".parse()?, 30, None)?],
                vec![NetworkAddress::new(
                    "10.44.0.2".parse()?,
                    30,
                    Some("192.0.2.1".parse()?),
                )?],
                &program,
                SetupBudget::begin().cutoff()?,
            )
            .unwrap_err();
        failure.cleanup()?;
        assert!(!Path::new("/sys/class/net/cg-host").exists());
        assert!(!Path::new("/sys/class/net/cg-peer").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        println!("NETWORK_CONFIGURATION_FAILURE_CLEANUP_PASS");
        fs::remove_dir(parent)?;
        println!("NETWORK_EXPIRED_SETUP_ROLLBACK_PASS");
        Ok(())
    }
}
