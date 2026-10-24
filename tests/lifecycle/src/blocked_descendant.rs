// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

/// Benign file-I/O fixture. Only the external trusted VM test suspends storage.
#[derive(Debug)]
pub(super) struct BlockedDescendant;
impl BlockedDescendant {
    pub(super) const SCRIPT: &str = r#"
trap 'exit 0' USR1
(
    exec 3>/blocked-writer
    kill -STOP "$BASHPID"
    printf 'blocked write\n' >&3
    exec /usr/bin/sync /blocked-writer
) &
child=$!
printf 'BLOCKED_RECIPE_READY %s %s\n' "$BASHPID" "$child" >&2
wait -f "$child"
exit 91
"#;

    pub(super) fn stage(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        for file in fs::read_to_string("/recipe-runtime-files")?.lines() {
            let destination = root.join(file);
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::copy(Path::new("/").join(file), destination)?;
        }
        fs::create_dir_all(root.join("bin"))?;
        std::os::unix::fs::symlink("/usr/bin/bash", root.join("bin/sh"))?;
        Ok(())
    }

    pub(super) fn host_pid(inner: u32) -> Result<u32, Box<dyn std::error::Error>> {
        let mut matches = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(status) = fs::read_to_string(entry.path().join("status")) else {
                continue;
            };
            if status.lines().any(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                fields.len() == 3 && fields[0] == "NSpid:" && fields[2].parse::<u32>() == Ok(inner)
            }) {
                matches.push(pid);
            }
        }
        match matches.as_slice() {
            [pid] => Ok(*pid),
            _ => Err(
                format!("expected unique fixture namespace PID {inner}, got {matches:?}").into(),
            ),
        }
    }

    pub(super) fn await_state(pid: u32, state: char) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
            if status
                .lines()
                .any(|line| line.starts_with(&format!("State:\t{state}")))
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    format!("fixture PID {pid} did not enter state {state}: {status}").into(),
                );
            }
            std::thread::yield_now();
        }
    }

    pub(super) fn signal(pid: u32, signal: &str) -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            std::process::Command::new("/usr/bin/bash")
                .args([
                    "-c",
                    "kill -s \"$1\" \"$2\"",
                    "fixture",
                    signal,
                    &pid.to_string()
                ])
                .status()?
                .success()
        );
        Ok(())
    }
}
