use super::algorithm::Algorithm;
use super::otp_type::{Code, OtpError, OtpInput, OtpType};
use super::params::{resolve, ParamKind, ParamSpec, Params, Resolved};
use crate::secret::Secret;

pub(super) const ALGORITHM: ParamSpec = ParamSpec {
    name: "algorithm",
    kind: ParamKind::Choice(Algorithm::NAMES),
    default: "SHA1",
};

pub(super) const DIGITS: ParamSpec = ParamSpec {
    name: "digits",
    kind: ParamKind::Integer { min: 6, max: 8 },
    default: "6",
};

static SCHEMA: [ParamSpec; 3] = [
    ALGORITHM,
    DIGITS,
    ParamSpec {
        name: "counter",
        kind: ParamKind::Integer {
            min: 0,
            max: u64::MAX,
        },
        default: "0",
    },
];

/// RFC 4226. The stored `counter` parameter is the entry's current value; the caller
/// passes it as [`OtpInput::Counter`].
pub struct Hotp;

impl OtpType for Hotp {
    fn id(&self) -> &'static str {
        "hotp"
    }

    fn schema(&self) -> &'static [ParamSpec] {
        &SCHEMA
    }

    fn generate(
        &self,
        secret: &Secret,
        params: &Params,
        input: OtpInput,
    ) -> Result<Code, OtpError> {
        let OtpInput::Counter(counter) = input else {
            return Err(OtpError::WrongInput {
                expected: "a counter",
            });
        };
        let resolved = resolve(&SCHEMA, params)?;
        hotp_code(secret, &Shared::from_resolved(&resolved), counter)
    }
}

/// Parameters every HMAC-based type shares.
pub(super) struct Shared {
    algorithm: Algorithm,
    digits: u32,
}

impl Shared {
    pub(super) fn from_resolved(resolved: &Resolved) -> Self {
        Self {
            algorithm: Algorithm::from_name(resolved.choice(ALGORITHM.name))
                .expect("algorithm choices come from Algorithm::NAMES"),
            digits: u32::try_from(resolved.integer(DIGITS.name))
                .expect("digits are bounded by the schema"),
        }
    }
}

pub(super) fn hotp_code(secret: &Secret, shared: &Shared, counter: u64) -> Result<Code, OtpError> {
    let truncated = truncated_value(secret, shared.algorithm, counter)?;
    let value = truncated % 10_u32.pow(shared.digits);
    Ok(Code::new(format!(
        "{value:0width$}",
        width = shared.digits as usize
    )))
}

/// RFC 4226 dynamic truncation: the 31-bit integer every HMAC-based code is derived from.
pub(super) fn truncated_value(
    secret: &Secret,
    algorithm: Algorithm,
    counter: u64,
) -> Result<u32, OtpError> {
    if secret.as_bytes().is_empty() {
        return Err(OtpError::EmptySecret);
    }
    let hash = algorithm.hmac(secret.as_bytes(), &counter.to_be_bytes());
    let offset = usize::from(hash[hash.len() - 1] & 0x0f);
    Ok(u32::from_be_bytes([
        hash[offset] & 0x7f,
        hash[offset + 1],
        hash[offset + 2],
        hash[offset + 3],
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc_secret() -> Secret {
        Secret::new(b"12345678901234567890".to_vec())
    }

    #[test]
    fn matches_every_rfc_4226_appendix_d_vector() {
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, code) in (0_u64..).zip(expected) {
            let generated = Hotp
                .generate(&rfc_secret(), &Params::new(), OtpInput::Counter(counter))
                .unwrap();
            assert_eq!(generated.as_str(), code, "counter {counter}");
        }
    }

    #[test]
    fn honours_the_digits_parameter_and_keeps_leading_zeros() {
        let params = Params::new().with("digits", "8");
        let code = Hotp
            .generate(&rfc_secret(), &params, OtpInput::Counter(0))
            .unwrap();
        assert_eq!(code.as_str().len(), 8);
        assert!(code.as_str().ends_with("755224"));
    }

    #[test]
    fn rejects_time_input() {
        let result = Hotp.generate(&rfc_secret(), &Params::new(), OtpInput::Time(59));
        assert_eq!(
            result,
            Err(OtpError::WrongInput {
                expected: "a counter"
            })
        );
    }

    #[test]
    fn rejects_an_empty_secret() {
        let result = Hotp.generate(
            &Secret::new(Vec::new()),
            &Params::new(),
            OtpInput::Counter(0),
        );
        assert_eq!(result, Err(OtpError::EmptySecret));
    }

    #[test]
    fn rejects_invalid_parameters() {
        let params = Params::new().with("digits", "4");
        assert!(matches!(
            Hotp.generate(&rfc_secret(), &params, OtpInput::Counter(0)),
            Err(OtpError::Params(_))
        ));
    }
}
