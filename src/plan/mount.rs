// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mount {
    /// Leave the underlying image directory visible at this exact path.
    None,
    Esp,
    Tmpfs,
}

#[cfg(test)]
mod tests {
    use super::Mount;

    #[test]
    fn none_is_an_explicit_string_not_null() {
        assert_eq!(
            serde_json::from_str::<Mount>("\"none\"").unwrap(),
            Mount::None
        );
        assert_eq!(serde_json::to_string(&Mount::None).unwrap(), "\"none\"");
        assert!(serde_json::from_str::<Mount>("null").is_err());
    }
}
