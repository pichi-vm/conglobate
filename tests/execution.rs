// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::execute::Step;
use std::{fs, path::Path};

mod common;
use common::install;

#[test]
#[ignore = "requires root and Linux mount/PID/network namespaces"]
fn root_readonly_context_network_and_descendant_lifetime() {
    let dir = conglobate::mounts::Scratch::new().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    install(&root, "/bin/sh");
    install(&root, "/bin/sleep");
    install(&root, "/usr/bin/stat");
    let context = dir.path().join("context");
    fs::create_dir(&context).unwrap();
    fs::write(context.join("input"), "original").unwrap();
    let mut step = Step {
        expected_parent: None,
        root: root.clone(),
        context: context.clone(),
        network: false,
        carapace: Some("abcd".into()),
        command: r#"
        test "$CARAPACE" = abcd
        test -z "${CONGLOBATE_HOST_SECRET+x}"
        if read -r inherited_input; then exit 43; fi
        for device in null zero random urandom; do
            test "$(stat -c %a /dev/$device)" = 666
        done
        while read -r name real effective rest; do
            if test "$name" = 'Uid:'; then test "$real" = 0; test "$effective" = 0; fi
        done < /proc/self/status
        if echo bad > /context/input; then exit 42; fi
        while IFS= read -r line; do printf '%s\n' "$line"; done < /proc/net/dev > /network
        (sleep 1; echo escaped > /leaked) &
        "#
        .into(),
    };
    let first = std::process::Command::new(env!("CARGO_BIN_EXE_conglobate"))
        .arg("__step")
        .arg("--expected-parent")
        .arg(fs::read_link("/proc/self").unwrap())
        .arg("--root")
        .arg(&root)
        .arg("--context")
        .arg(&context)
        .args(["--carapace", "abcd", "--"])
        .arg(&step.command)
        .env("CONGLOBATE_HOST_SECRET", "must-not-leak")
        .stdin(fs::File::open(context.join("input")).unwrap())
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        first.stdout.is_empty(),
        "recipe diagnostics must use stderr"
    );
    assert_eq!(
        fs::read_to_string(context.join("input")).unwrap(),
        "original"
    );
    let isolated = fs::read_to_string(root.join("network")).unwrap();
    assert_eq!(
        isolated.lines().filter(|line| line.contains(':')).count(),
        1
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert!(!root.join("leaked").exists());
    step.network = true;
    step.command =
        "while IFS= read -r line; do printf '%s\\n' \"$line\"; done < /proc/net/dev > /network"
            .into();
    step.run(Path::new(env!("CARGO_BIN_EXE_conglobate")))
        .unwrap();
    let inherited = fs::read_to_string(root.join("network")).unwrap();
    let interfaces = |text: &str| {
        text.lines()
            .filter_map(|line| line.split_once(':').map(|(name, _)| name.trim().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        interfaces(&inherited),
        interfaces(&fs::read_to_string("/proc/net/dev").unwrap())
    );
    step.carapace = None;
    step.command = "echo \"$CARAPACE\"".into();
    assert!(
        step.run(Path::new(env!("CARGO_BIN_EXE_conglobate")))
            .is_err()
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_conglobate"))
        .args(["__step", "--expected-parent", "0", "--root"])
        .arg(&root)
        .arg("--context")
        .arg(&context)
        .args(["--", "echo orphan > /orphan"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("parent exited"));
    assert!(!root.join("orphan").exists());
}
