// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    io,
    time::{Duration, Instant},
};

/// Absolute CLOCK_MONOTONIC cutoff shared by trusted helpers within one VM.
/// Helpers must remain in the same time namespace. This is a private launch
/// value, not a portable timestamp or an authenticated authorization token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetupCutoff(u64);
impl SetupCutoff {
    fn now() -> io::Result<u64> {
        let stamp = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        let seconds = u64::try_from(stamp.tv_sec).map_err(io::Error::other)?;
        seconds
            .checked_mul(1_000_000_000)
            .and_then(|value| value.checked_add(stamp.tv_nsec as u64))
            .ok_or_else(|| io::Error::other("monotonic timestamp overflow"))
    }
    pub(super) fn from_deadline(deadline: Instant) -> io::Result<Self> {
        // Sample the destination clock first: time spent sampling only shortens
        // the exported budget, never extends it.
        let monotonic = Self::now()?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "setup cutoff expired"))?;
        let nanos = u64::try_from(remaining.as_nanos()).map_err(io::Error::other)?;
        Ok(Self(monotonic.checked_add(nanos).ok_or_else(|| {
            io::Error::other("setup cutoff overflow")
        })?))
    }
    pub(crate) fn deadline(&self) -> io::Result<Instant> {
        // Sample the destination Instant first for the same conservative reason.
        let instant = Instant::now();
        let remaining = self
            .0
            .checked_sub(Self::now()?)
            .filter(|value| *value != 0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "setup cutoff expired"))?;
        instant
            .checked_add(Duration::from_nanos(remaining))
            .ok_or_else(|| io::Error::other("setup Instant overflow"))
    }
    /// Hex-encoded deterministic CBOR [version, monotonic nanoseconds] for a
    /// trusted helper argv. Re-forward these bytes; do not export a new budget.
    pub(crate) fn to_argument(self) -> String {
        let mut encoder = minicbor::Encoder::new(Vec::new());
        encoder
            .array(2)
            .expect("Vec writer")
            .u8(1)
            .expect("Vec writer")
            .u64(self.0)
            .expect("Vec writer");
        hex::encode(encoder.into_writer())
    }
    pub(crate) fn from_argument(text: &str) -> io::Result<Self> {
        if text.len() > 24 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized setup cutoff",
            ));
        }
        let bytes =
            hex::decode(text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut decoder = minicbor::Decoder::new(&bytes);
        let parsed = (|| -> Result<Self, minicbor::decode::Error> {
            if decoder.array()? != Some(2) || decoder.u8()? != 1 {
                return Err(minicbor::decode::Error::message(
                    "invalid setup cutoff version/shape",
                ));
            }
            Ok(Self(decoder.u64()?))
        })()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if decoder.position() != bytes.len() || parsed.to_argument() != text {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "noncanonical setup cutoff",
            ));
        }
        Ok(parsed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarding_never_renews_the_original_budget() {
        let original = Instant::now() + Duration::from_secs(30);
        let cutoff = SetupCutoff::from_deadline(original).unwrap();
        let wire = cutoff.to_argument();
        for _ in 0..100 {
            let forwarded = SetupCutoff::from_argument(&wire).unwrap();
            assert_eq!(forwarded, cutoff);
            assert!(forwarded.deadline().unwrap() <= original);
            assert_eq!(forwarded.to_argument(), wire);
        }
    }
    #[test]
    fn expired_or_malformed_cutoffs_fail_without_sleeping() {
        assert_eq!(
            SetupCutoff(0).deadline().unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(SetupCutoff::from_deadline(Instant::now()).is_err());
        for text in [
            "",
            "820200",
            "82010000",
            "82011800",
            "9f0100ff",
            "8201ff",
            "zz",
            "82011B0000000000000000",
        ] {
            assert!(SetupCutoff::from_argument(text).is_err(), "{text}");
        }
    }
}
