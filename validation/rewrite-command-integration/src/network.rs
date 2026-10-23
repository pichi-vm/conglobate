// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::{
        CommandDevices, CommandSetup, ExecutionGate, GatedCommandSetup, IdRange, Mapped,
        UserNamespace,
    },
    network::{InterfaceName, NetworkAddress, NetworkNamespace, VethPair},
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
    pub(crate) fn entry(
        gated: bool,
        expected: u64,
        traffic: bool,
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
            }
        }
        if traffic {
            for (bind, remote, gateway) in [
                ("0.0.0.0:0", "198.51.100.42:7", "10.44.0.1:45454"),
                ("[::]:0", "[2001:db8::42]:7", "[fd42:44::1]:45454"),
            ] {
                let socket = std::net::UdpSocket::bind(bind)?;
                socket.connect(remote)?;
                assert!(!socket.local_addr()?.ip().is_unspecified());
                socket.send_to(b"private-network", gateway)?;
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
    fn entry_roundtrip(
        program: &HelperProgram,
        network: &NetworkNamespace,
        traffic: bool,
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
            let setup = CommandSetup::new(
                fs::File::open(parent.join(user.record().name()))?,
                CommandDevices::create(100000, 100000)?,
            )?
            .with_network(network)?;
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
                .stdin(input)
                .spawn()?;
            let status = Process::from(child)
                .wait_until(Instant::now() + Duration::from_secs(10))?
                .status();
            assert!(status.success());
            for listener in &listeners {
                listener.set_read_timeout(Some(Duration::from_secs(3)))?;
                let mut bytes = [0; 32];
                let count = listener.recv(&mut bytes)?;
                assert_eq!(&bytes[..count], b"private-network");
            }
            drop(gate);
        }
        user.release()?;
        drop(recovery);
        fs::remove_dir(parent)?;
        println!("NETWORK_COMMAND_ENTRY_AND_DESCRIPTOR_CLOSURE_PASS");
        Ok(())
    }
    pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
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
        Self::entry_roundtrip(&program, &first, false)?;
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
        Self::entry_roundtrip(&program, configured.namespace(), true)?;
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
