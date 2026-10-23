// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    network::{InterfaceName, NetworkNamespace, VethPair},
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
        connection.close()?;
        assert!(!Path::new("/sys/class/net/cg-host").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("NETWORK_PEER_NAMESPACE_HANDOFF_CLEANUP_PASS");
        fs::remove_dir(parent)?;
        println!("NETWORK_EXPIRED_SETUP_ROLLBACK_PASS");
        Ok(())
    }
}
