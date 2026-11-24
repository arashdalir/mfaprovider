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

const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Base32Error {
    #[error("the text contains a character that is not base32")]
    InvalidCharacter,
    #[error("the text has an impossible length for base32")]
    InvalidLength,
    #[error("the text is empty")]
    Empty,
}

impl Secret {
    /// Decodes RFC 4648 base32 as authenticator apps show it: case-insensitive, with spaces,
    /// hyphens and `=` padding ignored.
    ///
    /// # Errors
    /// On an empty text, a non-base32 character, or a length no byte string encodes to.
    pub fn from_base32(text: &str) -> Result<Self, Base32Error> {
        let mut values = Zeroizing::new(Vec::with_capacity(text.len()));
        for character in text.bytes() {
            match character {
                b' ' | b'-' | b'=' => {}
                _ => values.push(base32_value(character).ok_or(Base32Error::InvalidCharacter)?),
            }
        }
        if values.is_empty() {
            return Err(Base32Error::Empty);
        }
        if !matches!(values.len() % 8, 0 | 2 | 4 | 5 | 7) {
            return Err(Base32Error::InvalidLength);
        }
        let mut bytes = Vec::with_capacity(values.len() * 5 / 8);
        let (mut buffer, mut bits) = (0_u16, 0_u8);
        for value in values.iter() {
            buffer = (buffer << 5) | u16::from(*value);
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                bytes.push((buffer >> bits).to_le_bytes()[0]);
                buffer &= (1 << bits) - 1;
            }
        }
        Ok(Self::new(bytes))
    }

    /// The seed as unpadded upper-case base32, wiped when dropped.
    #[must_use]
    pub fn to_base32(&self) -> Zeroizing<String> {
        let mut text = Zeroizing::new(String::with_capacity(self.as_bytes().len() * 8 / 5 + 1));
        let (mut buffer, mut bits) = (0_u16, 0_u8);
        for byte in self.as_bytes() {
            buffer = (buffer << 8) | u16::from(*byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                text.push(char::from(
                    BASE32_ALPHABET[usize::from((buffer >> bits) & 31)],
                ));
            }
            buffer &= (1 << bits) - 1;
        }
        if bits > 0 {
            text.push(char::from(
                BASE32_ALPHABET[usize::from((buffer << (5 - bits)) & 31)],
            ));
        }
        text
    }
}

fn base32_value(character: u8) -> Option<u8> {
    let upper = character.to_ascii_uppercase();
    BASE32_ALPHABET
        .iter()
        .position(|candidate| *candidate == upper)
        .and_then(|position| u8::try_from(position).ok())
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl PartialEq for Secret {
    /// Compares in time independent of where the bytes first differ.
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.as_bytes(), other.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

impl Eq for Secret {}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4648 section 10 test vectors.
    const BASE32_VECTORS: [(&str, &str); 7] = [
        ("f", "MY"),
        ("fo", "MZXQ"),
        ("foo", "MZXW6"),
        ("foob", "MZXW6YQ"),
        ("fooba", "MZXW6YTB"),
        ("foobar", "MZXW6YTBOI"),
        ("12345678901234567890", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
    ];

    #[test]
    fn base32_matches_the_rfc_vectors_both_ways() {
        for (plain, encoded) in BASE32_VECTORS {
            let secret = Secret::from_base32(encoded).unwrap();
            assert_eq!(secret.as_bytes(), plain.as_bytes(), "{encoded}");
            assert_eq!(secret.to_base32().as_str(), encoded);
        }
    }

    #[test]
    fn base32_input_is_lenient_about_case_spacing_and_padding() {
        let expected = Secret::new(b"foobar".to_vec());
        for text in [
            "mzxw6ytboi",
            "MZXW 6YTB-OI",
            "MZXW6YTBOI======",
            " mzxw6ytboi ",
        ] {
            assert_eq!(Secret::from_base32(text).unwrap(), expected, "{text}");
        }
    }

    #[test]
    fn base32_rejects_bad_input() {
        assert_eq!(Secret::from_base32("").err(), Some(Base32Error::Empty));
        assert_eq!(Secret::from_base32(" - = ").err(), Some(Base32Error::Empty));
        assert_eq!(
            Secret::from_base32("MZXW6YT1").err(),
            Some(Base32Error::InvalidCharacter)
        );
        assert_eq!(
            Secret::from_base32("MZXW6YTBO").err(),
            Some(Base32Error::InvalidLength)
        );
        assert_eq!(
            Secret::from_base32("M").err(),
            Some(Base32Error::InvalidLength)
        );
    }

    #[test]
    fn base32_round_trips_every_length_and_byte_value() {
        let bytes: Vec<u8> = (0..=255).collect();
        for length in 1..bytes.len() {
            let secret = Secret::new(bytes[..length].to_vec());
            let decoded = Secret::from_base32(&secret.to_base32()).unwrap();
            assert_eq!(decoded, secret, "length {length}");
        }
    }

    #[test]
    fn debug_output_never_contains_the_seed() {
        let secret = Secret::new(b"supersecretseed".to_vec());
        let printed = format!("{secret:?} {secret:#?}");
        assert!(!printed.contains("supersecretseed"));
        assert!(printed.contains("redacted"));
    }

    #[test]
    fn equality_compares_length_and_content() {
        assert_eq!(Secret::new(vec![1, 2, 3]), Secret::new(vec![1, 2, 3]));
        assert_ne!(Secret::new(vec![1, 2, 3]), Secret::new(vec![1, 2, 4]));
        assert_ne!(Secret::new(vec![1, 2, 3]), Secret::new(vec![1, 2]));
    }

    #[test]
    fn exposes_the_bytes_it_was_given() {
        assert_eq!(Secret::new(vec![1, 2, 3]).as_bytes(), &[1, 2, 3]);
    }
}
