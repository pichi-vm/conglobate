// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]
mod blocked_descendant;
mod blocked_io;
mod cache_storage;
mod chain_storage;
mod clone_filter;
mod contained;
mod domain_publication;
mod formatter_failures;
mod gate_failure;
mod network;
mod runtime_scope;
mod sandbox_failures;
mod writeback;
use conglobate::execution::{HelperInvocation, HelperProgram};
use std::{ffi::OsStr, path::Path};

#[derive(Debug)]
struct Lifecycle;
impl Lifecycle {
    fn run() -> Result<(), Box<dyn std::error::Error>> {
        if !Path::new("/conglobate-disposable-validation-vm").is_file() {
            return Err("disposable guest required".into());
        }
        let mut args = std::env::args_os();
        if args.next().as_deref() == Some(OsStr::new("/usr/bin/formatter-stall")) {
            formatter_failures::FormatterFailures::stall();
        }
        let args: Vec<_> = args.collect();
        if let Some(invocation) = HelperInvocation::parse(&args)? {
            invocation.execute(&HelperProgram::current()?);
        }
        match args.as_slice() {
            [mode] if mode == "sandbox-unmapped-owner" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::UnmappedOwner,
            ),
            [mode, cutoff] if mode == "sandbox-unmapped-owner-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::InspectOwners,
                )
            }
            [mode] if mode == "runtime-scope" => runtime_scope::RuntimeScope::run(),
            [mode] if mode == "sandbox-delayed-teardown" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::DelayedTeardown,
            ),
            [mode] if mode == "sandbox-blocked-descendant" => {
                sandbox_failures::SandboxFailures::run(
                    sandbox_failures::FailurePoint::BlockedDescendant,
                )
            }
            [mode] if mode == "sandbox-cleanup-timeout" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::CleanupTimeout,
            ),
            [mode, cutoff] if mode == "sandbox-cleanup-timeout-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::Never,
                )
            }
            [mode] if mode == "sandbox-held-cleanup" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::HeldCleanup)
            }
            [mode] if mode == "sandbox-missing-cleanup" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::MissingCleanup,
            ),
            [mode, cutoff] if mode == "sandbox-held-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::Held,
                )
            }
            [mode, cutoff] if mode == "sandbox-missing-cleanup-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::Missing,
                )
            }
            [mode] if mode == "clone-filter" => clone_filter::CloneFilter::run(),
            [mode] if mode == "sandbox-busy-execution" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::BusyExecution,
            ),
            [mode, cutoff] if mode == "sandbox-success-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::Normal,
                )
            }
            [mode] if mode == "chain-storage" => chain_storage::ChainStorage::run(),
            [mode] if mode == "cache-storage" => cache_storage::CacheStorage::run(),
            [mode] if mode == "sandbox-writeback" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Writeback)
            }
            [mode] if mode == "sandbox-supervisor-death" => sandbox_failures::SandboxFailures::run(
                sandbox_failures::FailurePoint::SupervisorDeath,
            ),
            [mode] if mode == "sandbox-blocked-io" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::BlockedIo)
            }
            [mode, cutoff] if mode == "sandbox-blocked-helper" => {
                blocked_io::BlockedIo::helper(cutoff)
            }
            [mode] if mode == "sandbox-gate-failure" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Gate)
            }
            [mode, cutoff] if mode == "sandbox-gate-receiver" => {
                gate_failure::GateFailure::receiver(cutoff)
            }
            [mode] if mode == "sandbox-gate-child" => gate_failure::GateFailure::child(),
            [mode, interface, port] if mode == "network-capture" => {
                network::NetworkProbe::capture(interface, port)
            }
            [mode] if mode == "network-service" => network::NetworkProbe::service(),
            [mode] if mode == "network-composite" => network::NetworkProbe::composite_failures(),
            [mode, contended] if mode == "network-allocation-lock-child" => {
                match contended.to_str() {
                    Some("true") => network::NetworkProbe::allocation_lock_child(true),
                    Some("false") => network::NetworkProbe::allocation_lock_child(false),
                    _ => Err("expected lock contention boolean".into()),
                }
            }
            [mode, cutoff] if mode == "sandbox-attached-helper" => {
                sandbox_failures::SandboxFailures::attached_helper(
                    cutoff,
                    sandbox_failures::HelperCompletion::None,
                )
            }
            [mode] if mode == "sandbox-launch-failure" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Launch)
            }
            [mode] if mode == "sandbox-join-failure" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Join)
            }
            [mode] if mode == "sandbox-attached-exit" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::AttachedExit)
            }
            [mode] if mode == "sandbox-namespace-failure" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Namespace)
            }
            [mode] if mode == "sandbox-mount-failure" => {
                sandbox_failures::SandboxFailures::run(sandbox_failures::FailurePoint::Mount)
            }
            [mode] if mode == "formatter-failures" => formatter_failures::FormatterFailures::run(),
            [mode] if mode == "domain-publication" => domain_publication::DomainPublication::run(),
            [mode] if mode == "contained" => contained::Contained::run(),
            [mode] if mode == "contained-helper" => {
                contained::Contained::helper(contained::HelperMode::Wait)
            }
            [mode] if mode == "contained-reject" => {
                contained::Contained::helper(contained::HelperMode::RejectJoin)
            }
            [mode] if mode == "contained-abort" => {
                contained::Contained::helper(contained::HelperMode::Abort)
            }
            [mode] if mode == "contained-child" => contained::Contained::child(),
            _ => Err("expected lifecycle case or trusted helper invocation".into()),
        }
    }
}
fn main() {
    if let Err(error) = Lifecycle::run() {
        eprintln!("lifecycle fixture: {error}");
        std::process::exit(1);
    }
}
