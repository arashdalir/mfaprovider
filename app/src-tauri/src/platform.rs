//! Production time, randomness and key-derivation tuning for the core's injected traits.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mfa_core::vault::{Clock, KdfParams, Rng, run_kdf_benchmark, tune_kdf_params};

/// How long unlocking should take on this device, and the most memory it may use (kept
/// modest so the same vault also opens on low-end phones).
const KDF_TARGET: Duration = Duration::from_millis(500);
const KDF_MAX_MEMORY_KIB: u32 = 256 * 1024;

#[derive(Clone)]
pub struct SystemClock {
    started: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn unix_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
            .unwrap_or(0)
    }

    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Operating-system randomness. There is no safe fallback, so a failure stops the program.
pub struct OsRng;

impl Rng for OsRng {
    fn fill_bytes(&mut self, destination: &mut [u8]) {
        getrandom::fill(destination).expect("the operating system provides randomness");
    }
}

/// Measures this device and picks key-derivation parameters for a new vault.
pub fn tuned_kdf_params() -> KdfParams {
    tune_kdf_params(KDF_TARGET, KDF_MAX_MEMORY_KIB, |params| {
        let started = Instant::now();
        // Parameters come from the tuner, which only produces valid ones.
        let _ = run_kdf_benchmark(params);
        started.elapsed()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_monotonic_clock_never_goes_backwards() {
        let clock = SystemClock::default();
        let first = clock.monotonic_ms();
        assert!(clock.monotonic_ms() >= first);
    }

    #[test]
    fn wall_clock_time_is_after_2024() {
        assert!(SystemClock::default().unix_ms() > 1_700_000_000_000);
    }

    #[test]
    fn os_randomness_fills_the_buffer_and_differs_between_calls() {
        let (mut first, mut second) = ([0_u8; 32], [0_u8; 32]);
        OsRng.fill_bytes(&mut first);
        OsRng.fill_bytes(&mut second);
        assert_ne!(first, [0_u8; 32]);
        assert_ne!(first, second);
    }

    #[test]
    fn tuned_parameters_respect_the_memory_cap_and_the_format_floor() {
        let params = tuned_kdf_params();
        assert!(params.memory_kib() <= KDF_MAX_MEMORY_KIB);
        assert!(params.memory_kib() >= KdfParams::MINIMUM.memory_kib());
        assert!(params.iterations() >= KdfParams::MINIMUM.iterations());
    }
}
