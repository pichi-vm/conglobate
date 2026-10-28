// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::copy::CopyMode;

#[test]
fn valid_internal_modes_remain_available_without_plan_fields() {
    for mode in ["0644", "u=rw,g=u,o=r", "a+X", "u+x-w", "a+t", "a=", "u=go"] {
        assert!(mode.parse::<CopyMode>().is_ok(), "{mode}");
    }
}
#[test]
fn invalid_modes_fail_before_filesystem_execution() {
    for text in [
        "", "888", "10000", "+755", "-755", "u", "a+z", "a+r,", "u+ r", "0o755",
    ] {
        assert!(text.parse::<CopyMode>().is_err(), "{text}");
    }
}
