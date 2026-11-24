/// Time source, injected so the core does no I/O and tests control time.
pub trait Clock {
    /// Wall-clock time in Unix milliseconds, for timestamps stored in the vault.
    fn unix_ms(&self) -> i64;

    /// Milliseconds on a monotonic clock with an arbitrary origin, for idle timing. Wall-clock
    /// jumps (time zone, NTP, a user changing the date) must not lock or unlock anything.
    fn monotonic_ms(&self) -> u64;
}
