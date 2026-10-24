// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    path::{ContextPath, PathError},
    value::{CommandTimeout, FilesystemLabel, FilesystemSize},
};
use std::{path::Path, time::Duration};

#[test]
fn context_relative_paths_do_not_gain_resolution_authority() {
    for path in ["../recipe", "a/../../lock"] {
        assert_eq!(
            ContextPath::try_from(Path::new(path)),
            Err(PathError::Escape)
        );
    }
    assert_eq!(
        ContextPath::try_from(Path::new("/recipe")),
        Err(PathError::Absolute)
    );
    assert_eq!(
        ContextPath::try_from(Path::new("a/..")),
        Err(PathError::Empty)
    );
    assert_eq!(
        ContextPath::try_from(Path::new("a/../recipes/build.yml"))
            .unwrap()
            .as_ref(),
        Path::new("recipes/build.yml")
    );
}

#[test]
fn capacity_is_explicit_aligned_and_bounded() {
    let minimum = 16 * 1024 * 1024;
    let maximum = FilesystemSize::LOGICAL_DEVICE_BYTES;
    for bytes in [minimum, minimum + 4096, maximum - 4096, maximum] {
        let numeric = FilesystemSize::try_from(bytes).unwrap();
        assert_eq!(numeric.bytes(), bytes);
        assert_eq!(format!("{bytes}B").parse::<FilesystemSize>(), Ok(numeric));
    }
    for bytes in [0, minimum - 4096, minimum + 1, maximum + 4096, u64::MAX] {
        assert!(FilesystemSize::try_from(bytes).is_err(), "{bytes}");
    }
    assert_eq!(
        "4GiB".parse::<FilesystemSize>().unwrap().blocks(),
        1_048_576
    );
    assert_eq!(
        "64GiB".parse::<FilesystemSize>().unwrap().bytes(),
        FilesystemSize::LOGICAL_DEVICE_BYTES
    );
    for value in [
        "0B",
        "1GiB ",
        "1.5GiB",
        "-1GiB",
        "4GB",
        "4",
        "15MiB",
        "65GiB",
        "16777217B",
        "18446744073709551615GiB",
    ] {
        assert!(value.parse::<FilesystemSize>().is_err(), "{value}");
    }
}

#[test]
fn labels_use_encoded_bytes_not_character_count() {
    assert!(FilesystemLabel::try_from("é".repeat(8)).is_ok());
    assert!(FilesystemLabel::try_from("é".repeat(9)).is_err());
    assert!(FilesystemLabel::try_from("a\0b".to_owned()).is_err());
}

#[test]
fn commands_require_a_finite_positive_timeout() {
    assert_eq!(
        CommandTimeout::default().duration(),
        Duration::from_secs(300)
    );
    assert_eq!(
        "30m".parse::<CommandTimeout>().unwrap().duration(),
        Duration::from_secs(1800)
    );
    assert_eq!(
        "1ms".parse::<CommandTimeout>().unwrap().duration(),
        Duration::from_millis(1)
    );
    for value in ["0s", "infinite", "5", "-1s", "18446744073709551615h"] {
        assert!(value.parse::<CommandTimeout>().is_err(), "{value}");
    }
}

#[test]
fn timeout_parsing_uses_duration_bounds_without_a_policy_limit() {
    assert_eq!(
        format!("{}ms", u64::MAX)
            .parse::<CommandTimeout>()
            .unwrap()
            .duration(),
        Duration::from_millis(u64::MAX)
    );
    let seconds = u64::MAX / 1000;
    assert_eq!(
        format!("{seconds}s")
            .parse::<CommandTimeout>()
            .unwrap()
            .duration(),
        Duration::from_secs(seconds)
    );
    assert_eq!(
        format!("{}s", seconds + 1)
            .parse::<CommandTimeout>()
            .unwrap_err(),
        conglobate::value::ValueError::Overflow
    );
}
