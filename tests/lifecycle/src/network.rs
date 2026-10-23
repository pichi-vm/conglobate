// SPDX-License-Identifier: Apache-2.0
use conglobate::{execution::HelperProgram, network::FirewallTable, step::SetupBudget};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct NetworkProbe;
impl NetworkProbe {
    pub(super) fn allocation_lock_child(contended: bool) -> Result<(), Box<dyn std::error::Error>> {
        let namespace = fs::File::open("/proc/self/ns/net")?;
        let result = rustix::fs::flock(
            &namespace,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        );
        if contended {
            assert_eq!(result.unwrap_err(), rustix::io::Errno::WOULDBLOCK);
        } else {
            result?;
        }
        Ok(())
    }
    fn allocation_lock_probe() -> Result<(), Box<dyn std::error::Error>> {
        let namespace = fs::File::open("/proc/self/ns/net")?;
        rustix::fs::flock(
            &namespace,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )?;
        let other = fs::File::open("/proc/self/ns/net")?;
        assert_eq!(
            rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive)
                .unwrap_err(),
            rustix::io::Errno::WOULDBLOCK
        );
        assert!(
            std::process::Command::new(std::env::current_exe()?)
                .arg("network-allocation-lock-child")
                .arg("true")
                .status()?
                .success()
        );
        for (namespace, contended) in [("--mount", "true"), ("--net", "false")] {
            assert!(
                std::process::Command::new("unshare")
                    .arg(namespace)
                    .arg(std::env::current_exe()?)
                    .arg("network-allocation-lock-child")
                    .arg(contended)
                    .status()?
                    .success()
            );
        }
        drop(namespace);
        rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        println!("NETWORK_NAMESPACE_FLOCK_EXCLUSION_AND_RELEASE_PASS");
        Ok(())
    }
    pub(super) fn composite_failures() -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            network::{CommandNetwork, ResolverInput},
            storage::Workspace,
        };
        Self::allocation_lock_probe()?;
        let workspace = Workspace::create()?;
        let program = HelperProgram::current()?;
        let parent = Path::new("/work/network-composite");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let resolver = parent.join("resolver-input");
        fs::write(&resolver, b"nameserver 192.0.2.53\n")?;
        let resolver = ResolverInput::capture(&resolver)?;
        fs::remove_file(parent.join("resolver-input"))?;
        // Fixture-only configuration in the NIC-less disposable VM.
        fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
        fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1")?;
        let held = fs::File::open("/proc/self/ns/net")?;
        rustix::fs::flock(&held, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        assert!(failure.to_string().contains("network allocation lease"));
        failure.cleanup()?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        drop(held);
        println!("NETWORK_PRODUCTION_ALLOCATION_CONTENTION_BEFORE_MUTATION_PASS");
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            &parent.join("missing"),
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        failure.cleanup()?;
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("NETWORK_COMPOSITE_AFTER_NAMESPACE_FAILURE_CLEANUP_PASS");

        let foreign =
            FirewallTable::create("conglobate_cgb0h", Instant::now() + Duration::from_secs(10))?;
        let failure = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )
        .unwrap_err();
        assert!(failure.to_string().contains("NAT installation"));
        failure.cleanup()?;
        assert!(
            std::process::Command::new("nft")
                .args(["list", "table", "inet", "conglobate_cgb0h"])
                .output()?
                .status
                .success()
        );
        assert!(!Path::new("/sys/class/net/cgb0h").exists());
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        foreign.close(Instant::now() + Duration::from_secs(10))?;
        println!("NETWORK_COMPOSITE_NAT_FAILURE_PRESERVES_FOREIGN_TABLE_PASS");

        let network = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )?;
        let second = CommandNetwork::prepare(
            &resolver,
            &workspace,
            parent,
            &program,
            &SetupBudget::begin(),
        )?;
        assert!(Path::new("/sys/class/net/cgb1h").exists());
        let second_address = std::net::UdpSocket::bind("10.0.0.5:0")?;
        drop(second_address);
        second.close(Instant::now() + Duration::from_secs(10))?;
        assert!(!Path::new("/sys/class/net/cgb1h").exists());
        assert!(Path::new("/sys/class/net/cgb0h").exists());
        println!("NETWORK_PRODUCTION_ALLOCATION_RELEASE_AND_DISTINCT_LIVE_PREFIX_PASS");
        let mut failure = network.close(Instant::now()).unwrap_err();
        for _ in 0..2 {
            assert!(
                failure
                    .diagnostics()
                    .to_string()
                    .contains("deadline expired")
            );
            assert!(Path::new("/sys/class/net/cgb0h").exists());
            assert!(
                std::process::Command::new("nft")
                    .args(["list", "table", "inet", "conglobate_cgb0h"])
                    .output()?
                    .status
                    .success()
            );
            assert_eq!(fs::read_dir(parent)?.count(), 1);
            failure = failure.retry().unwrap_err();
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        println!("NETWORK_COMPOSITE_EXPIRED_TEARDOWN_RETRY_RETAINS_OWNER_PASS");
        // This is deliberately the final probe. No successful cleanup is
        // claimed: the VM controller disposes of retained resources on exit.
        drop(failure);
        Ok(())
    }
}
