use zeroize::Zeroizing;

/// Source of cryptographically secure random bytes, injected so the core does no I/O
/// and tests can supply fixed values. Production code backs this with OS randomness.
pub trait Rng {
    fn fill_bytes(&mut self, destination: &mut [u8]);
}

pub(super) fn random_array<const N: usize>(rng: &mut dyn Rng) -> [u8; N] {
    let mut bytes = [0_u8; N];
    rng.fill_bytes(&mut bytes);
    bytes
}

pub(super) fn random_key(rng: &mut dyn Rng) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0_u8; 32]);
    rng.fill_bytes(&mut *key);
    key
}
