use super::algorithm::Algorithm;
use super::hotp::truncated_value;
use super::otp_type::{Code, CodeTiming, OtpError, OtpInput, OtpType};
use super::params::{resolve, ParamError, ParamSpec, Params};
use crate::secret::Secret;

const ALPHABET: &[u8; 26] = b"23456789BCDFGHJKMNPQRTVWXY";
const CODE_LENGTH: usize = 5;
const PERIOD_SECONDS: u64 = 30;

/// Steam Guard: the TOTP truncation of HMAC-SHA1 over 30 s steps, written as five
/// base-26 characters instead of decimal digits. Algorithm, length and period are
/// fixed, so the schema declares no parameters.
pub struct Steam;

impl OtpType for Steam {
    fn id(&self) -> &'static str {
        "steam"
    }

    fn schema(&self) -> &'static [ParamSpec] {
        &[]
    }

    fn timing(&self, params: &Params) -> Result<CodeTiming, ParamError> {
        resolve(self.schema(), params)?;
        Ok(CodeTiming::Time {
            period_seconds: PERIOD_SECONDS,
        })
    }

    fn generate(
        &self,
        secret: &Secret,
        params: &Params,
        input: OtpInput,
    ) -> Result<Code, OtpError> {
        let OtpInput::Time(unix_seconds) = input else {
            return Err(OtpError::WrongInput { expected: "a time" });
        };
        resolve(self.schema(), params)?;
        let mut remaining =
            truncated_value(secret, Algorithm::Sha1, unix_seconds / PERIOD_SECONDS)?;
        let characters = (0..CODE_LENGTH).map(|_| {
            let character = ALPHABET[(remaining % 26) as usize];
            remaining /= 26;
            char::from(character)
        });
        Ok(Code::new(characters.collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected codes are derived independently of this implementation: the integers
    /// are the "decimal" truncated values published in RFC 4226 Appendix D for the key
    /// `12345678901234567890` (their last six digits are the RFC's own HOTP codes,
    /// which cross-checks them), written in base 26 with Steam's alphabet,
    /// least-significant character first. Counter `n` corresponds to time `n * 30`.
    ///
    /// No official Steam vectors exist; add a real-account fixture if one becomes
    /// available.
    const VECTORS: [(u64, &str); 10] = [
        (0, "GG5F5"),
        (1, "PV9M4"),
        (2, "B26KJ"),
        (3, "5H85C"),
        (4, "6Y9J3"),
        (5, "MD224"),
        (6, "P2GRF"),
        (7, "C9PRW"),
        (8, "3NKKN"),
        (9, "5YCKB"),
    ];

    fn secret() -> Secret {
        Secret::new(b"12345678901234567890".to_vec())
    }

    fn at(time: u64) -> Result<Code, OtpError> {
        Steam.generate(&secret(), &Params::new(), OtpInput::Time(time))
    }

    #[test]
    fn matches_independently_derived_vectors() {
        for (counter, expected) in VECTORS {
            assert_eq!(
                at(counter * 30).unwrap().as_str(),
                expected,
                "counter {counter}"
            );
        }
    }

    #[test]
    fn uses_the_whole_30_second_step() {
        assert_eq!(at(30).unwrap(), at(59).unwrap());
        assert_ne!(at(59).unwrap(), at(60).unwrap());
    }

    #[test]
    fn codes_have_five_characters_from_the_steam_alphabet() {
        for time in (0..3000).step_by(30) {
            let code = at(time).unwrap();
            assert_eq!(code.as_str().len(), CODE_LENGTH);
            assert!(code.as_str().bytes().all(|c| ALPHABET.contains(&c)));
        }
    }

    #[test]
    fn rejects_every_parameter_because_the_type_is_fixed() {
        for name in ["digits", "period", "algorithm"] {
            let params = Params::new().with(name, "x");
            let result = Steam.generate(&secret(), &params, OtpInput::Time(0));
            assert!(matches!(result, Err(OtpError::Params(_))), "{name}");
        }
    }

    #[test]
    fn rejects_counter_input() {
        let result = Steam.generate(&secret(), &Params::new(), OtpInput::Counter(0));
        assert_eq!(result, Err(OtpError::WrongInput { expected: "a time" }));
    }

    #[test]
    fn rejects_an_empty_secret() {
        let result = Steam.generate(&Secret::new(Vec::new()), &Params::new(), OtpInput::Time(0));
        assert_eq!(result, Err(OtpError::EmptySecret));
    }
}
