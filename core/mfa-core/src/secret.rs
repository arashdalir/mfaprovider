use std::fmt;

use zeroize::Zeroizing;

/// Raw OTP seed bytes. Zeroized on drop; never printed, never cloned.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_seed() {
        let secret = Secret::new(b"supersecretseed".to_vec());
        let printed = format!("{secret:?} {secret:#?}");
        assert!(!printed.contains("supersecretseed"));
        assert!(printed.contains("redacted"));
    }

    #[test]
    fn exposes_the_bytes_it_was_given() {
        assert_eq!(Secret::new(vec![1, 2, 3]).as_bytes(), &[1, 2, 3]);
    }
}
