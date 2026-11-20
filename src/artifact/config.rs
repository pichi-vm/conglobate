// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! The artifact **config blob** (`application/vnd.pichi.config.v1+json`) — the
//! image-level metadata carried in the OCI manifest `config` descriptor
//! (BUILD.md §7.1). It holds the **launch contract** (`requirements`): what the
//! host must provide to launch the artifact.
//!
//! The blob is self-identifying: a top-level `version` (currently `1`) states
//! the schema, so meaning changes (e.g. the memory unit) are unambiguous rather
//! than silently reinterpreted. Sizes are integers in the units named on each
//! field (memory in **MiB**), not raw bytes.
//!
//! The `Requirements` model (and its `Band`/`PortSpec`/`Interface`/`Ingress`
//! types) lives here — this module owns the artifact's config schema. It is
//! JSON on the wire; the human-authored `pichi.yaml` supplies the requirements
//! that the builder writes into this config.

use std::collections::BTreeMap;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The config-blob schema version carried in-body. Bumped when the meaning of a
/// field changes; consumers reject anything they don't understand.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// The pichi artifact config blob (`application/vnd.pichi.config.v1+json`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Schema version (see [`CONFIG_SCHEMA_VERSION`]). Required on the wire so
    /// the blob is self-identifying independent of the media-type suffix.
    pub version: u32,

    /// The host-facing launch contract.
    #[serde(default)]
    pub requirements: Requirements,
}

// Canonical field order and requirements are properties of the wire format,
// independent of the caller's choice of JSON or another serde codec.
impl Serialize for Config {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let mut requirements = self.requirements.clone();
        requirements.canonicalize();
        let mut state = serializer.serialize_struct("Config", 2)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("requirements", &requirements)?;
        state.end()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_SCHEMA_VERSION,
            requirements: Requirements::default(),
        }
    }
}

/// Errors constructing or validating a [`Config`].
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The blob declares a schema version this build does not understand.
    #[error("unsupported config schema version {got} (this build understands {want})")]
    UnsupportedVersion {
        /// The version found in the blob.
        got: u32,
        /// The version this build supports.
        want: u32,
    },
}

impl Config {
    /// Check the schema version is understood. Requirement invariants (nonzero,
    /// 2-MiB memory, band ordering) are enforced by the field types at parse, so
    /// there is nothing else to check here. Callers validate after decoding.
    ///
    /// # Errors
    /// [`ConfigError::UnsupportedVersion`] if the version is not understood.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                got: self.version,
                want: CONFIG_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}

/// The host-facing launch contract (BUILD.md §7): what the host MUST provide to
/// launch a pichi artifact: resource sizing, networking and declared device
/// transports. Storage describes one mutable overlay above the read-only image.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    /// Default transport for attached devices, MMIO when omitted by the plan.
    /// A resource's own declaration takes precedence. Launch must realize it.
    #[serde(default)]
    pub slot: Slot,

    /// vCPU band. The floor is 1 (a VM always has at least one vCPU), so a
    /// value of `1` carries no information and is canonicalised away; omit the
    /// band entirely to leave sizing to the operator / dillo default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<Band<NonZeroU32>>,

    /// Guest-memory band. Each tier is a [`Memory`] — MiB quantised to 2 MiB
    /// hugepages, nonzero — so a non-conforming size is unrepresentable rather
    /// than caught by a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Band<Memory>>,

    /// Capacity and transport for one mutable overlay; defaults to a 1024 MiB
    /// recommendation without a required minimum.
    #[serde(default, skip_serializing_if = "Storage::is_default")]
    pub storage: Storage,

    /// Network interfaces the host must wire (BUILD.md §7). One user-mode
    /// virtio-net device per entry; the map key is the operator-facing label.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub interfaces: BTreeMap<String, Interface>,

    /// Required host directory shares, keyed by public virtiofs tag.
    /// Guest mount points and host source paths are not image metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub filesystems: BTreeMap<String, Filesystem>,
}

/// A required virtiofs share. The host binds its name; the guest mounts it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Filesystem {
    /// Human-facing description of the directory the operator must supply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Require host-enforced read-only access; omission requires writable access.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub readonly: bool,

    /// Declared transport; omission inherits the image's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
}

/// Declared virtio transport; the PMI platform must provide matching slots.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    /// Require PCI.
    Pci,

    /// Require virtio-MMIO; the plan/config default.
    #[default]
    Mmio,
}

impl<'de> Deserialize<'de> for Slot {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match String::deserialize(d)?.as_str() {
            "pci" => Ok(Self::Pci),
            "mmio" => Ok(Self::Mmio),
            _ => Err(serde::de::Error::custom("expected \"pci\" or \"mmio\"")),
        }
    }
}

/// The single mutable overlay's capacity in MiB and declared transport.
/// Backing paths, retention and provisioning are operator configuration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Storage {
    /// Positive MiB capacities; unlike memory, storage has no 2-MiB constraint.
    #[serde(flatten)]
    pub capacity: Band<NonZeroU64>,

    /// Declared transport; omission inherits the global default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            capacity: Band {
                required: None,
                recommended: Some(Recommendation::Value(Self::DEFAULT_RECOMMENDED)),
            },
            slot: None,
        }
    }
}

impl Storage {
    const DEFAULT_RECOMMENDED: NonZeroU64 = NonZeroU64::new(1024).unwrap();

    fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

impl<'de> Deserialize<'de> for Storage {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Decode a closed shape before constructing the checked band. Flattening
        // deserialization would mix the band's unknown-field policy with slot.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            required: Option<NonZeroU64>,
            recommended: Option<Recommendation<NonZeroU64>>,
            slot: Option<Slot>,
        }
        let wire = Wire::deserialize(d)?;
        let recommended = wire.recommended.or(Some(Recommendation::Value(
            wire.required
                .unwrap_or(Self::DEFAULT_RECOMMENDED)
                .max(Self::DEFAULT_RECOMMENDED),
        )));
        let capacity = Band::new(wire.required, recommended)
            .ok_or_else(|| serde::de::Error::custom("recommended is below required"))?;
        Ok(Self {
            capacity,
            slot: wire.slot,
        })
    }
}

/// One declared network interface (BUILD.md §7).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    /// Human-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Ports the host must/should expose inbound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<Ingress>,

    /// Declared transport; omission inherits the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
}

/// Inbound port bands for an [`Interface`] (BUILD.md §7).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ingress {
    /// Ports the host MUST expose, else launch errors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<PortSpec>,

    /// Ports the host SHOULD expose, else a warning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended: Vec<PortSpec>,
}

/// One ingress port-list entry: a single port, an inclusive `[low, high]`
/// range, or `all` (all ports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSpec {
    /// A single port.
    Port(u16),

    /// An inclusive `[low, high]` range.
    Range(u16, u16),

    /// All ports (`all`).
    All,
}

impl Serialize for PortSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PortSpec::Port(p) => s.serialize_u16(*p),
            PortSpec::Range(lo, hi) => {
                use serde::ser::SerializeSeq;
                let mut seq = s.serialize_seq(Some(2))?;
                seq.serialize_element(lo)?;
                seq.serialize_element(hi)?;
                seq.end()
            }
            PortSpec::All => s.serialize_str("all"),
        }
    }
}

impl<'de> Deserialize<'de> for PortSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = PortSpec;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a port (integer), an inclusive [low, high] range, or \"all\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<PortSpec, E> {
                u16::try_from(v)
                    .map(PortSpec::Port)
                    .map_err(|_| E::custom(format!("port {v} out of range")))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<PortSpec, E> {
                if v == "all" {
                    Ok(PortSpec::All)
                } else {
                    Err(E::custom(format!(
                        "expected a port, range, or \"all\", got {v:?}"
                    )))
                }
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<PortSpec, A::Error> {
                use serde::de::Error as _;
                let lo: u16 = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("range needs [low, high]"))?;
                let hi: u16 = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("range needs [low, high]"))?;
                if seq.next_element::<u16>()?.is_some() {
                    return Err(A::Error::custom("range has more than two elements"));
                }
                Ok(PortSpec::Range(lo, hi))
            }
        }
        d.deserialize_any(V)
    }
}

/// A recommendation is either a concrete value or the largest allocation the
/// operator permits. `Max` must resolve to a concrete value before launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recommendation<T> {
    /// Prefer this value, which is also the default allocation.
    Value(T),

    /// Prefer the largest allocation permitted by operator and platform limits.
    Max,
}

impl<T> Recommendation<T> {
    /// Convert the numeric value without losing a `max` recommendation.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Recommendation<U> {
        match self {
            Self::Value(value) => Recommendation::Value(f(value)),
            Self::Max => Recommendation::Max,
        }
    }
}

impl<T: Serialize> Serialize for Recommendation<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(s),
            Self::Max => s.serialize_str("max"),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Recommendation<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire<T> {
            Value(T),
            Keyword(String),
        }
        match Wire::deserialize(d)? {
            Wire::Value(value) => Ok(Self::Value(value)),
            Wire::Keyword(value) if value == "max" => Ok(Self::Max),
            Wire::Keyword(_) => Err(serde::de::Error::custom("expected a number or \"max\"")),
        }
    }
}

/// A two-tier sizing requirement. The recommendation supplies the default;
/// a manual value below `required` errors, or below a numeric `recommended`
/// warns. Neither is a ceiling; both are individually optional.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, bound(serialize = "T: Serialize"))]
pub struct Band<T> {
    // Private: the ordering invariant (`recommended ≥ required`) is enforced by
    // `new`/`Deserialize`, so bands can only be built through a checked path.
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recommended: Option<Recommendation<T>>,
}

// Manual `Default` (the derive would wrongly require `T: Default`; an empty band
// is `None`/`None` regardless of `T`, so `NonZeroU32`/`Memory` bands work).
impl<T> Default for Band<T> {
    fn default() -> Self {
        Self {
            required: None,
            recommended: None,
        }
    }
}

impl<T: PartialOrd> Band<T> {
    /// Build a band, or `None` if `recommended` is below `required` — the sole
    /// *relational* invariant (it relates two values, so it can't be a value
    /// newtype). This is the one place it's checked; [`Deserialize`] routes
    /// through it, so any band you hold — from code or the wire — is ordered.
    #[must_use]
    pub fn new(required: Option<T>, recommended: Option<Recommendation<T>>) -> Option<Self> {
        if let (Some(req), Some(Recommendation::Value(rec))) = (&required, &recommended)
            && rec < req
        {
            return None;
        }
        Some(Self {
            required,
            recommended,
        })
    }
}

impl<T: Copy> Band<T> {
    /// The `required` tier, if set.
    #[must_use]
    pub fn required(self) -> Option<T> {
        self.required
    }

    /// The `recommended` tier, if set.
    #[must_use]
    pub fn recommended(self) -> Option<Recommendation<T>> {
        self.recommended
    }
}

impl<T> Band<T> {
    /// True when neither tier is set (nothing to say).
    fn is_empty(&self) -> bool {
        self.required.is_none() && self.recommended.is_none()
    }
}

/// Deserialization mirror: parse the two tiers, then route through
/// [`Band::new`] so the ordering invariant holds for wire input too.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct BandWire<T> {
    #[serde(default)]
    required: Option<T>,
    #[serde(default)]
    recommended: Option<Recommendation<T>>,
}

impl<'de, T> Deserialize<'de> for Band<T>
where
    T: Deserialize<'de> + PartialOrd,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = BandWire::<T>::deserialize(d)?;
        Band::new(w.required, w.recommended)
            .ok_or_else(|| serde::de::Error::custom("recommended is below required"))
    }
}

/// Guest memory in MiB, quantised to whole 2 MiB hugepages. dillo backs guest
/// RAM with 2 MiB hugepages, so memory exists only in 2 MiB chunks; this newtype
/// makes a non-conforming size *unrepresentable* — construct only via
/// [`Memory::from_mib`] (nonzero and a multiple of [`Memory::CHUNK_MIB`]), and
/// the wire form (a MiB integer) is rejected at parse if it isn't. The invariant
/// then holds everywhere without a separate validation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Memory(NonZeroU32);

impl Memory {
    /// The hugepage chunk memory is quantised to, in MiB.
    pub const CHUNK_MIB: u32 = 2;

    /// Build from a MiB size, or `None` if it is zero or not a multiple of
    /// [`Self::CHUNK_MIB`].
    #[must_use]
    pub fn from_mib(mib: u32) -> Option<Self> {
        if mib == 0 || !mib.is_multiple_of(Self::CHUNK_MIB) {
            return None;
        }
        NonZeroU32::new(mib).map(Self)
    }

    /// The size in MiB (always a positive multiple of [`Self::CHUNK_MIB`]).
    #[must_use]
    pub fn mib(self) -> u32 {
        self.0.get()
    }
}

impl Serialize for Memory {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.0.get())
    }
}

impl<'de> Deserialize<'de> for Memory {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mib = u32::deserialize(d)?;
        Self::from_mib(mib).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "memory {mib} MiB must be a positive multiple of {} (2 MiB hugepages)",
                Self::CHUNK_MIB
            ))
        })
    }
}

impl Requirements {
    /// Canonicalise for content-addressed emission: drop values that carry no
    /// information so semantically-equal contracts serialise identically.
    /// Currently: `cpus` has a hard floor of 1, so a `required`/`recommended`
    /// of `1` means "the floor" — indistinguishable from "unset" — and is
    /// removed; an emptied `cpus` band is dropped entirely.
    pub fn canonicalize(&mut self) {
        if let Some(b) = self.cpus.as_mut() {
            let floor = NonZeroU32::MIN; // 1
            if b.required == Some(floor) {
                b.required = None;
            }
            if b.recommended == Some(Recommendation::Value(floor)) {
                b.recommended = None;
            }
            if b.is_empty() {
                self.cpus = None;
            }
        }
    }

    /// Required vCPU floor, if declared (absent ⇒ the implicit floor of 1).
    #[must_use]
    pub fn cpus_required(&self) -> Option<u32> {
        self.cpus.and_then(|b| b.required).map(NonZeroU32::get)
    }

    /// Recommended vCPU count, if declared.
    #[must_use]
    pub fn cpus_recommended(&self) -> Option<Recommendation<u32>> {
        self.cpus
            .and_then(|b| b.recommended)
            .map(|r| r.map(NonZeroU32::get))
    }

    /// Required memory floor in MiB, if declared.
    #[must_use]
    pub fn memory_required_mib(&self) -> Option<u32> {
        self.memory.and_then(|b| b.required).map(Memory::mib)
    }

    /// Recommended memory in MiB, if declared.
    #[must_use]
    pub fn memory_recommended_mib(&self) -> Option<Recommendation<u32>> {
        self.memory
            .and_then(|b| b.recommended)
            .map(|r| r.map(Memory::mib))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    recommended: Some(Recommendation::Value(nz(4))),
                }),
                memory: Some(Band {
                    required: Memory::from_mib(2048),
                    recommended: Memory::from_mib(4096).map(Recommendation::Value),
                }),
                ..Default::default()
            },
        };
        let b = serde_json::to_vec(&c).unwrap();
        let c2 = serde_json::from_slice::<Config>(&b).unwrap();
        assert_eq!(c, c2);
        assert_eq!(c2.requirements.cpus_required(), Some(2));
        assert_eq!(
            c2.requirements.cpus_recommended(),
            Some(Recommendation::Value(4))
        );
        assert_eq!(c2.requirements.memory_required_mib(), Some(2048));
        assert_eq!(
            c2.requirements.memory_recommended_mib(),
            Some(Recommendation::Value(4096))
        );
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
            r#"{"version":1,"requirements":{"slot":"mmio"}}"#
        );
        // recommended==1 is likewise the floor, but a real recommended survives.
        let c2 = Config {
            requirements: Requirements {
                cpus: Some(Band {
                    required: Some(nz(1)),
                    recommended: Some(Recommendation::Value(nz(4))),
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let back = serde_json::from_slice::<Config>(&serde_json::to_vec(&c2).unwrap()).unwrap();
        assert_eq!(back.requirements.cpus_required(), None); // floor dropped
        assert_eq!(
            back.requirements.cpus_recommended(),
            Some(Recommendation::Value(4))
        );
    }

    #[test]
    fn rejects_recommended_below_required() {
        // The constructor rejects it — the relational invariant lives on the type...
        assert!(Band::new(Some(nz(4)), Some(Recommendation::Value(nz(2)))).is_none());
        assert!(Band::new(Some(nz(2)), Some(Recommendation::Value(nz(4)))).is_some());
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
            recommended: vec![PortSpec::Port(80), PortSpec::All],
        };
        let j = serde_json::to_string(&ing).unwrap();
        assert_eq!(
            j,
            r#"{"required":[443,[8000,8999]],"recommended":[80,"all"]}"#
        );
        let back: Ingress = serde_json::from_str(&j).unwrap();
        assert_eq!(back, ing);
    }

    #[test]
    fn port_wildcard_stays_an_entry_in_either_list() {
        for tier in ["required", "recommended"] {
            let value = serde_json::json!({tier: [443, "all", [8000, 8100]]});
            let ingress: Ingress = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(ingress).unwrap(), value);
            for invalid in [
                serde_json::json!("all"),
                serde_json::json!(["*"]),
                serde_json::json!(["max"]),
            ] {
                assert!(
                    serde_json::from_value::<Ingress>(serde_json::json!({tier: invalid})).is_err()
                );
            }
        }
    }

    #[test]
    fn slot_types_are_typed_and_default_to_mmio() {
        for slot in ["pci", "mmio"] {
            let value = serde_json::json!({
                "slot": slot,
                "interfaces": {"public": {"slot": slot}}
            });
            let requirements: Requirements = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                Some(requirements.slot),
                requirements.interfaces["public"].slot
            );
            assert_eq!(serde_json::to_value(requirements).unwrap(), value);
        }
        for slot in [
            serde_json::json!("pcie"),
            serde_json::json!(0),
            serde_json::json!({"pci": null}),
            serde_json::json!(["pci"]),
            serde_json::json!(true),
        ] {
            for value in [
                serde_json::json!({"slot": slot}),
                serde_json::json!({"interfaces": {"public": {"slot": slot}}}),
                serde_json::json!({"filesystems": {"data": {"slot": slot}}}),
            ] {
                assert!(serde_json::from_value::<Requirements>(value).is_err());
            }
        }
        let requirements: Requirements = serde_json::from_str("{}").unwrap();
        assert_eq!(requirements.slot, Slot::Mmio);
        assert_eq!(
            serde_json::to_string(&requirements).unwrap(),
            r#"{"slot":"mmio"}"#
        );
        let config: Config = serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert_eq!(config.requirements.slot, Slot::Mmio);
        assert_eq!(config, Config::default());
    }

    #[test]
    fn filesystem_requirements_round_trip_without_private_paths() {
        let value = serde_json::json!({"version": 1, "requirements": {
            "slot": "pci",
            "filesystems": {
                "repo": {
                    "description": "Git repository containing the build source.",
                    "readonly": true,
                    "slot": "mmio"
                },
                "output": {}
            }
        }});
        let config: Config = serde_json::from_value(value.clone()).unwrap();
        assert!(config.requirements.filesystems["repo"].readonly);
        assert_eq!(
            config.requirements.filesystems["repo"].slot,
            Some(Slot::Mmio)
        );
        assert_eq!(
            config.requirements.filesystems["output"],
            Filesystem::default()
        );
        assert_eq!(serde_json::to_value(config).unwrap(), value);

        for private_field in ["mountpoint", "source"] {
            let invalid = serde_json::json!({"version": 1, "requirements": {
                "filesystems": {"repo": {private_field: "/private"}}
            }});
            assert!(serde_json::from_value::<Config>(invalid).is_err());
        }
    }

    #[test]
    fn omitted_storage_recommendation_defaults_at_parse() {
        for value in [
            serde_json::json!({"version": 1}),
            serde_json::json!({"version": 1, "requirements": {}}),
            serde_json::json!({"version": 1, "requirements": {"storage": {}}}),
        ] {
            let config: Config = serde_json::from_value(value).unwrap();
            assert_eq!(config, Config::default());
            assert_eq!(
                config.requirements.storage.capacity.recommended(),
                Some(Recommendation::Value(NonZeroU64::new(1024).unwrap()))
            );
        }
        for (value, expected) in [
            (serde_json::json!({"required": 1}), 1024),
            (serde_json::json!({"required": 2048}), 2048),
            (serde_json::json!({"recommended": 2}), 2),
        ] {
            let storage: Storage = serde_json::from_value(value).unwrap();
            assert_eq!(
                storage.capacity.recommended(),
                Some(Recommendation::Value(NonZeroU64::new(expected).unwrap()))
            );
        }
        #[cfg(feature = "yaml")]
        {
            let yaml: Requirements = serde_saphyr::from_str("storage: {}\n").unwrap();
            assert_eq!(yaml.storage, Storage::default());
        }
    }

    #[test]
    fn storage_describes_one_overlay_in_positive_mib() {
        for storage in [
            serde_json::json!({"recommended": 1024, "slot": "mmio"}),
            serde_json::json!({"required": 1, "recommended": 3, "slot": "pci"}),
            serde_json::json!({"recommended": u64::MAX}),
        ] {
            let value = serde_json::json!({"slot":"mmio", "storage": storage});
            let requirements: Requirements = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(requirements).unwrap(), value);
        }
        for storage in [
            serde_json::json!({"required": 0}),
            serde_json::json!({"recommended": 0}),
            serde_json::json!({"required": -1}),
            serde_json::json!({"required": 1.5}),
            serde_json::json!({"required": 4, "recommended": 2}),
            serde_json::json!({"slot": "pcie"}),
            serde_json::json!({"data": {"required": 1024}}),
            serde_json::json!({"path": "/tmp/overlay"}),
        ] {
            assert!(
                serde_json::from_value::<Requirements>(serde_json::json!({"storage": storage}))
                    .is_err()
            );
        }
    }

    #[test]
    fn max_recommendations_round_trip_without_becoming_omitted() {
        let value = serde_json::json!({"version": 1, "requirements": {
            "slot": "mmio",
            "cpus": {"required": 2, "recommended": "max"},
            "memory": {"required": 2048, "recommended": "max"},
            "storage": {"required": 10240, "recommended": "max", "slot": "pci"}
        }});
        let config: Config = serde_json::from_value(value.clone()).unwrap();
        config.validate().unwrap();
        assert_eq!(
            config.requirements.cpus_recommended(),
            Some(Recommendation::Max)
        );
        assert_eq!(
            config.requirements.memory_recommended_mib(),
            Some(Recommendation::Max)
        );
        assert_eq!(
            config.requirements.storage.capacity.recommended(),
            Some(Recommendation::Max)
        );
        assert_eq!(serde_json::to_value(config).unwrap(), value);

        let config: Config = serde_json::from_str(
            r#"{"version":1,"requirements":{"cpus":{"required":1,"recommended":"max"}}}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&config).unwrap(),
            r#"{"version":1,"requirements":{"slot":"mmio","cpus":{"recommended":"max"}}}"#
        );
    }

    #[test]
    fn sizing_keeps_numeric_constraints_and_max_is_only_recommended() {
        for resource in ["cpus", "memory", "storage"] {
            for band in [
                serde_json::json!({"required": "max"}),
                serde_json::json!({"recommended": "all"}),
                serde_json::json!({"recommended": "*"}),
                serde_json::json!({"recommended": 0}),
                serde_json::json!({"recommended": -1}),
                serde_json::json!({"recommended": 1.5}),
                serde_json::json!({"required": 4, "recommended": 2}),
                serde_json::json!({"recommended": {"max": true}}),
                serde_json::json!({"recommended": {"max": null}}),
                serde_json::json!({"recommended": ["max"]}),
            ] {
                assert!(
                    serde_json::from_value::<Requirements>(serde_json::json!({resource: band}))
                        .is_err(),
                    "{resource}: {band}"
                );
            }
        }
        assert!(
            serde_json::from_value::<Requirements>(
                serde_json::json!({"memory": {"recommended": 3}})
            )
            .is_err()
        );
        for recommended in [
            None,
            Some(Recommendation::Value(nz(4))),
            Some(Recommendation::Max),
        ] {
            assert_eq!(
                Band::new(Some(nz(2)), recommended).unwrap().recommended(),
                recommended
            );
        }
    }
}
