//! Password → key derivation (Argon2id) with bounds for untrusted headers and tuning.

use std::time::Duration;

use argon2::{Algorithm, Argon2, Params, Version};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::error::VaultError;

pub(super) const KEY_LENGTH: usize = 32;
pub(super) const SALT_LENGTH: usize = 16;

/// Argon2id cost settings, always within the bounds of ADR-0011.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    memory_kib: u32,
    iterations: u32,
    lanes: u8,
}

impl KdfParams {
    /// OWASP minimums for Argon2id; also the floor accepted when reading a file.
    pub const MINIMUM: Self = Self {
        memory_kib: 19_456,
        iterations: 2,
        lanes: 1,
    };
    pub const MAX_MEMORY_KIB: u32 = 1_048_576;
    pub const MAX_ITERATIONS: u32 = 20;
    pub const MAX_LANES: u8 = 16;

    /// # Errors
    /// If a value is below the OWASP minimum or above the ceiling that protects against
    /// hostile files demanding unbounded memory or time.
    pub fn new(memory_kib: u32, iterations: u32, lanes: u8) -> Result<Self, VaultError> {
        let min = Self::MINIMUM;
        if memory_kib < min.memory_kib || iterations < min.iterations || lanes < min.lanes {
            return Err(VaultError::InvalidKdfParams("below the minimum cost"));
        }
        if memory_kib > Self::MAX_MEMORY_KIB
            || iterations > Self::MAX_ITERATIONS
            || lanes > Self::MAX_LANES
        {
            return Err(VaultError::InvalidKdfParams("above the maximum cost"));
        }
        Ok(Self {
            memory_kib,
            iterations,
            lanes,
        })
    }

    #[must_use]
    pub const fn memory_kib(&self) -> u32 {
        self.memory_kib
    }

    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }

    #[must_use]
    pub const fn lanes(&self) -> u8 {
        self.lanes
    }
}

/// Derives the 32-byte key-encryption key. The password is NFKC-normalised first so the
/// same text typed on different platforms yields the same key (ADR-0011).
pub(super) fn derive_key(
    password: &str,
    salt: &[u8; SALT_LENGTH],
    params: KdfParams,
) -> Result<Zeroizing<[u8; KEY_LENGTH]>, VaultError> {
    let normalised: Zeroizing<String> = Zeroizing::new(password.nfkc().collect());
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        u32::from(params.lanes),
        Some(KEY_LENGTH),
    )
    .map_err(|_| VaultError::InvalidKdfParams("rejected by Argon2"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut key = Zeroizing::new([0_u8; KEY_LENGTH]);
    argon2
        .hash_password_into(normalised.as_bytes(), salt, &mut *key)
        .map_err(|_| VaultError::InvalidKdfParams("rejected by Argon2"))?;
    Ok(key)
}

/// Picks parameters whose derivation takes at least `target` on this device.
///
/// Doubles memory first (up to `max_memory_kib`, itself clamped to the format limits),
/// then raises iterations. `measure` runs one derivation with the given parameters and
/// returns how long it took; timing is the caller's job so the core stays I/O-free.
/// If the target is unreachable the strongest allowed parameters are returned.
pub fn tune_kdf_params(
    target: Duration,
    max_memory_kib: u32,
    mut measure: impl FnMut(KdfParams) -> Duration,
) -> KdfParams {
    let memory_ceiling =
        max_memory_kib.clamp(KdfParams::MINIMUM.memory_kib, KdfParams::MAX_MEMORY_KIB);
    let mut params = KdfParams::MINIMUM;
    while measure(params) < target {
        if let Some(memory) = params
            .memory_kib
            .checked_mul(2)
            .filter(|memory| *memory <= memory_ceiling)
        {
            params.memory_kib = memory;
        } else if params.iterations < KdfParams::MAX_ITERATIONS {
            params.iterations += 1;
        } else {
            break;
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_values_inside_the_bounds() {
        assert!(KdfParams::new(19_456, 2, 1).is_ok());
        assert!(KdfParams::new(KdfParams::MAX_MEMORY_KIB, 20, 16).is_ok());
    }

    #[test]
    fn rejects_values_below_the_owasp_minimum() {
        for (m, t, p) in [(19_455, 2, 1), (19_456, 1, 1), (19_456, 2, 0)] {
            assert_eq!(
                KdfParams::new(m, t, p),
                Err(VaultError::InvalidKdfParams("below the minimum cost")),
                "{m} {t} {p}"
            );
        }
    }

    #[test]
    fn rejects_values_above_the_ceiling() {
        for (m, t, p) in [
            (KdfParams::MAX_MEMORY_KIB + 1, 2, 1),
            (19_456, 21, 1),
            (19_456, 2, 17),
            (u32::MAX, 2, 1),
        ] {
            assert_eq!(
                KdfParams::new(m, t, p),
                Err(VaultError::InvalidKdfParams("above the maximum cost")),
                "{m} {t} {p}"
            );
        }
    }

    #[test]
    fn composed_and_decomposed_passwords_derive_the_same_key() {
        let salt = [7_u8; SALT_LENGTH];
        let composed = derive_key("caf\u{e9}", &salt, KdfParams::MINIMUM).unwrap();
        let decomposed = derive_key("cafe\u{301}", &salt, KdfParams::MINIMUM).unwrap();
        assert_eq!(*composed, *decomposed);
    }

    #[test]
    fn different_passwords_or_salts_derive_different_keys() {
        let salt = [7_u8; SALT_LENGTH];
        let base = derive_key("password", &salt, KdfParams::MINIMUM).unwrap();
        assert_ne!(
            *base,
            *derive_key("Password", &salt, KdfParams::MINIMUM).unwrap()
        );
        assert_ne!(
            *base,
            *derive_key("password", &[8_u8; SALT_LENGTH], KdfParams::MINIMUM).unwrap()
        );
    }

    /// Fake clock: one derivation takes memory (KiB) × iterations microseconds.
    fn fake_cost(params: KdfParams) -> Duration {
        Duration::from_micros(u64::from(params.memory_kib()) * u64::from(params.iterations()))
    }

    #[test]
    fn tuning_stops_at_the_first_parameters_that_reach_the_target() {
        let target = Duration::from_millis(1000);
        let tuned = tune_kdf_params(target, 1_048_576, fake_cost);
        assert!(fake_cost(tuned) >= target);
        assert_eq!(tuned.memory_kib(), 19_456 * 32);
        assert_eq!(tuned.iterations(), 2);
        let half_as_much_memory = KdfParams::new(tuned.memory_kib() / 2, 2, 1).unwrap();
        assert!(
            fake_cost(half_as_much_memory) < target,
            "a cheaper setting already sufficed"
        );
    }

    #[test]
    fn tuning_never_goes_below_the_minimum() {
        let tuned = tune_kdf_params(Duration::ZERO, 1_048_576, fake_cost);
        assert_eq!(tuned, KdfParams::MINIMUM);
    }

    #[test]
    fn tuning_raises_iterations_once_memory_is_capped() {
        let target = Duration::from_millis(500);
        let tuned = tune_kdf_params(target, 19_456 * 2, fake_cost);
        assert_eq!(tuned.memory_kib(), 19_456 * 2);
        assert_eq!(tuned.iterations(), 13);
        assert!(fake_cost(tuned) >= target);
    }

    #[test]
    fn tuning_gives_up_at_the_strongest_allowed_parameters() {
        let tuned = tune_kdf_params(Duration::from_secs(3600), 19_456 * 2, fake_cost);
        assert_eq!(tuned.memory_kib(), 19_456 * 2);
        assert_eq!(tuned.iterations(), KdfParams::MAX_ITERATIONS);
    }

    #[test]
    fn tuning_respects_the_format_ceiling_even_if_asked_for_more() {
        let tuned = tune_kdf_params(Duration::from_secs(3600), u32::MAX, fake_cost);
        assert!(tuned.memory_kib() <= KdfParams::MAX_MEMORY_KIB);
    }
}
