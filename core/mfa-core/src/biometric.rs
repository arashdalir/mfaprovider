//! What the core needs from a platform's biometric facility (ADR-0003, ADR-0016).
//!
//! A provider may exist only where the operating system can bind a key to the biometric: it
//! keeps the vault key wrapped by such a key, and gives it back only after the user has
//! proved themselves. The core never learns how; it only hands over and gets back 32 bytes.

use thiserror::Error;
use zeroize::Zeroizing;

use crate::vault::VAULT_KEY_LENGTH;

pub type VaultKeyBytes = Zeroizing<[u8; VAULT_KEY_LENGTH]>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Biometrics can be enabled.
    Available,
    /// This platform has none, or the phone has no biometric set up.
    Unavailable,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum BiometricError {
    #[error("biometric unlock is not available on this device")]
    Unavailable,
    #[error("the biometric prompt was cancelled")]
    Cancelled,
    #[error("the biometric was not recognised")]
    Failed,
    /// The operating system dropped the key, for instance because a new fingerprint was added.
    #[error("the biometric key is no longer valid")]
    Invalidated,
    #[error("the platform's biometric facility failed")]
    Platform,
}

pub trait BiometricProvider: Send {
    fn availability(&self) -> Availability;

    /// Wraps `key` with a new OS-bound key, replacing any earlier wrap.
    ///
    /// # Errors
    /// [`BiometricError::Unavailable`] or [`BiometricError::Platform`].
    fn enrol(&mut self, key: &[u8; VAULT_KEY_LENGTH]) -> Result<(), BiometricError>;

    /// Asks the user to authenticate and returns the vault key.
    ///
    /// # Errors
    /// [`BiometricError::Cancelled`], [`BiometricError::Failed`], [`BiometricError::Invalidated`]
    /// (the wrap is gone and must be enrolled again) or [`BiometricError::Platform`].
    fn release(&mut self) -> Result<VaultKeyBytes, BiometricError>;

    /// Deletes the wrap and its OS key. Removing what is not there is not an error.
    fn remove(&mut self);
}

/// For platforms without a usable biometric: always unavailable.
pub struct NoBiometrics;

impl BiometricProvider for NoBiometrics {
    fn availability(&self) -> Availability {
        Availability::Unavailable
    }

    fn enrol(&mut self, _key: &[u8; VAULT_KEY_LENGTH]) -> Result<(), BiometricError> {
        Err(BiometricError::Unavailable)
    }

    fn release(&mut self) -> Result<VaultKeyBytes, BiometricError> {
        Err(BiometricError::Unavailable)
    }

    fn remove(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_platform_without_biometrics_refuses_everything() {
        let mut none = NoBiometrics;
        assert_eq!(none.availability(), Availability::Unavailable);
        assert_eq!(none.enrol(&[0; 32]), Err(BiometricError::Unavailable));
        assert_eq!(none.release().unwrap_err(), BiometricError::Unavailable);
        none.remove();
    }
}
