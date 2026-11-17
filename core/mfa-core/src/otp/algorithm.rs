use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    pub const NAMES: &'static [&'static str] = &["SHA1", "SHA256", "SHA512"];

    pub(super) fn from_name(name: &str) -> Option<Self> {
        match name {
            "SHA1" => Some(Self::Sha1),
            "SHA256" => Some(Self::Sha256),
            "SHA512" => Some(Self::Sha512),
            _ => None,
        }
    }

    pub(super) fn hmac(self, key: &[u8], message: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => compute::<Hmac<Sha1>>(key, message),
            Self::Sha256 => compute::<Hmac<Sha256>>(key, message),
            Self::Sha512 => compute::<Hmac<Sha512>>(key, message),
        }
    }
}

fn compute<M: Mac + KeyInit>(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = <M as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for name in Algorithm::NAMES {
            assert!(Algorithm::from_name(name).is_some(), "{name}");
        }
        assert_eq!(Algorithm::from_name("MD5"), None);
    }

    #[test]
    fn hmac_sha1_matches_rfc_2202_case_2() {
        let mac = Algorithm::Sha1.hmac(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            mac,
            [
                0xef, 0xfc, 0xdf, 0x6a, 0xe5, 0xeb, 0x2f, 0xa2, 0xd2, 0x74, 0x16, 0xd5, 0xf1, 0x84,
                0xdf, 0x9c, 0x25, 0x9a, 0x7c, 0x79
            ]
        );
    }
}
