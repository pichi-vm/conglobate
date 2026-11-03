// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Resource requirements shared by config files and artifact config blobs.

mod band;
mod config;
mod config_error;
mod ingress;
mod interface;
mod memory;
mod port_spec;
mod requirements;

pub use band::Band;
pub use config::{CONFIG_SCHEMA_VERSION, Config};
pub use config_error::ConfigError;
pub use ingress::Ingress;
pub use interface::Interface;
pub use memory::Memory;
pub use port_spec::PortSpec;
pub use requirements::Requirements;

/// `requirements.yaml` launch-contract layer (BUILD.md §7) — the host-facing
/// resource contract for a launchable artifact. A small uncompressed YAML
/// blob; at most one per manifest.
///
/// Superseded by the config blob ([`MEDIA_TYPE_PICHI_CONFIG_V1`]): the launch
/// contract now rides in the manifest config, not a layer. Retained during the
/// migration; new writers MUST NOT emit it.
pub const MEDIA_TYPE_PICHI_REQUIREMENTS_V1: &str = "application/vnd.pichi.requirements.v1+yaml";

/// pichi image config blob — the OCI 1.1 `config` descriptor's target. Carries
/// the launch contract (the `requirements` projection of `config.yaml`,
/// BUILD.md §7.1); replaces the OCI empty-config for pichi artifacts. JSON.
pub const MEDIA_TYPE_PICHI_CONFIG_V1: &str = "application/vnd.pichi.config.v1+json";

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, num::NonZeroU32};

    fn nz(v: u32) -> NonZeroU32 {
        NonZeroU32::new(v).unwrap()
    }

    #[test]
    fn empty_config_round_trips() {
        let c = Config::default();
        let b = serde_json::to_vec(&c).unwrap();
        assert_eq!(serde_json::from_slice::<Config>(&b).unwrap(), c);
    }

    #[test]
    fn requirements_round_trip_and_accessors() {
        let c = Config {
            version: CONFIG_SCHEMA_VERSION,
            requirements: Requirements {
                cpus: Some(Band {
                    required: Some(nz(2)),
                    recommended: Some(nz(4)),
                }),
                memory: Some(Band {
                    required: Memory::from_mib(2048),
                    recommended: Memory::from_mib(4096),
                }),
                interfaces: BTreeMap::new(),
            },
        };
        let b = serde_json::to_vec(&c).unwrap();
        let c2 = serde_json::from_slice::<Config>(&b).unwrap();
        assert_eq!(c, c2);
        assert_eq!(c2.requirements.cpus_required(), Some(2));
        assert_eq!(c2.requirements.cpus_recommended(), Some(4));
        assert_eq!(c2.requirements.memory_required_mib(), Some(2048));
        assert_eq!(c2.requirements.memory_recommended_mib(), Some(4096));
    }

    #[test]
    fn cpus_floor_is_canonicalized_away() {
        // required == 1 is the floor (a VM always has ≥1 vCPU) → dropped, and an
        // emptied band disappears, so this equals a config with no cpus at all.
        let c = Config {
            requirements: Requirements {
                cpus: Some(Band {
                    required: Some(nz(1)),
                    recommended: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let bytes = serde_json::to_vec(&c).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            r#"{"version":1,"requirements":{}}"#
        );
        // recommended==1 is likewise the floor, but a real recommended survives.
        let c2 = Config {
            requirements: Requirements {
                cpus: Some(Band {
                    required: Some(nz(1)),
                    recommended: Some(nz(4)),
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let back = serde_json::from_slice::<Config>(&serde_json::to_vec(&c2).unwrap()).unwrap();
        assert_eq!(back.requirements.cpus_required(), None); // floor dropped
        assert_eq!(back.requirements.cpus_recommended(), Some(4));
    }

    #[test]
    fn rejects_recommended_below_required() {
        // The constructor rejects it — the relational invariant lives on the type...
        assert!(Band::new(Some(nz(4)), Some(nz(2))).is_none());
        assert!(Band::new(Some(nz(2)), Some(nz(4))).is_some());
        // ...and Deserialize routes through `new`, so the wire is rejected too.
        let err = serde_json::from_slice::<Config>(
            br#"{"version":1,"requirements":{"cpus":{"required":4,"recommended":2}}}"#,
        );
        assert!(err.is_err());
    }

    #[test]
    fn rejects_zero_cpus_at_parse() {
        // Nonzero typing makes 0 unrepresentable — serde rejects it directly.
        let err = serde_json::from_slice::<Config>(
            br#"{"version":1,"requirements":{"cpus":{"required":0}}}"#,
        );
        assert!(err.is_err());
    }

    #[test]
    fn memory_must_be_2mib_aligned() {
        // The type makes a non-2-MiB size unrepresentable: no runtime check,
        // the constructor/parse refuses it.
        assert!(Memory::from_mib(2048).is_some());
        assert!(Memory::from_mib(2).is_some());
        assert!(Memory::from_mib(3).is_none()); // odd MiB
        assert!(Memory::from_mib(0).is_none()); // zero
        // ...and it's rejected at parse.
        let err = serde_json::from_slice::<Config>(
            br#"{"version":1,"requirements":{"memory":{"required":513}}}"#,
        );
        assert!(err.is_err());
    }

    #[test]
    fn rejects_unknown_version() {
        let config: Config = serde_json::from_slice(br#"{"version":2,"requirements":{}}"#).unwrap();
        let err = config.validate();
        assert!(matches!(
            err,
            Err(ConfigError::UnsupportedVersion { got: 2, want: 1 })
        ));
    }

    #[test]
    fn port_spec_forms() {
        let ing = Ingress {
            required: vec![PortSpec::Port(443), PortSpec::Range(8000, 8999)],
            recommended: vec![PortSpec::All],
        };
        let j = serde_json::to_string(&ing).unwrap();
        assert_eq!(j, r#"{"required":[443,[8000,8999]],"recommended":["*"]}"#);
        let back: Ingress = serde_json::from_str(&j).unwrap();
        assert_eq!(back, ing);
    }
}
