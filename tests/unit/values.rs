// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::path::{ContextPath, PathError};
use std::path::Path;

#[test]
fn context_relative_paths_do_not_gain_resolution_authority() {
    for path in ["../plan", "a/../../lock"] {
        assert_eq!(
            ContextPath::try_from(Path::new(path)),
            Err(PathError::Escape)
        );
    }
    assert_eq!(
        ContextPath::try_from(Path::new("/plan")),
        Err(PathError::Absolute)
    );
    assert_eq!(
        ContextPath::try_from(Path::new("a/..")),
        Err(PathError::Empty)
    );
    assert_eq!(
        ContextPath::try_from(Path::new("a/../plans/build.yml"))
            .unwrap()
            .as_ref(),
        Path::new("a/../plans/build.yml")
    );
}
