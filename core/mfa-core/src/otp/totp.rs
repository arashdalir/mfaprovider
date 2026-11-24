use super::hotp::{hotp_code, Shared, ALGORITHM, DIGITS};
use super::otp_type::{Code, CodeTiming, OtpError, OtpInput, OtpType};
use super::params::{resolve, ParamError, ParamKind, ParamSpec, Params};
use crate::secret::Secret;

const PERIOD: ParamSpec = ParamSpec {
    name: "period",
    kind: ParamKind::Integer { min: 1, max: 3600 },
    default: "30",
};

static SCHEMA: [ParamSpec; 3] = [ALGORITHM, DIGITS, PERIOD];

/// RFC 6238: HOTP over `unix_time / period`.
pub struct Totp;

impl OtpType for Totp {
    fn id(&self) -> &'static str {
        "totp"
    }

    fn schema(&self) -> &'static [ParamSpec] {
        &SCHEMA
    }

    fn timing(&self, params: &Params) -> Result<CodeTiming, ParamError> {
        let resolved = resolve(&SCHEMA, params)?;
        Ok(CodeTiming::Time {
            period_seconds: resolved.integer(PERIOD.name),
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
        let resolved = resolve(&SCHEMA, params)?;
        let period = resolved.integer(PERIOD.name);
        hotp_code(
            secret,
            &Shared::from_resolved(&resolved),
            unix_seconds / period,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMES: [u64; 6] = [
        59,
        1_111_111_109,
        1_111_111_111,
        1_234_567_890,
        2_000_000_000,
        20_000_000_000,
    ];

    fn check(algorithm: &str, seed: &[u8], expected: [&str; 6]) {
        let secret = Secret::new(seed.to_vec());
        let params = Params::new()
            .with("algorithm", algorithm)
            .with("digits", "8");
        for (time, code) in TIMES.into_iter().zip(expected) {
            let generated = Totp
                .generate(&secret, &params, OtpInput::Time(time))
                .unwrap();
            assert_eq!(generated.as_str(), code, "{algorithm} at t={time}");
        }
    }

    #[test]
    fn matches_rfc_6238_vectors_for_sha1() {
        check(
            "SHA1",
            b"12345678901234567890",
            [
                "94287082", "07081804", "14050471", "89005924", "69279037", "65353130",
            ],
        );
    }

    #[test]
    fn matches_rfc_6238_vectors_for_sha256() {
        check(
            "SHA256",
            b"12345678901234567890123456789012",
            [
                "46119246", "68084774", "67062674", "91819424", "90698825", "77737706",
            ],
        );
    }

    #[test]
    fn matches_rfc_6238_vectors_for_sha512() {
        check(
            "SHA512",
            b"1234567890123456789012345678901234567890123456789012345678901234",
            [
                "90693936", "25091201", "99943326", "93441116", "38618901", "47863826",
            ],
        );
    }

    #[test]
    fn code_is_stable_within_a_period_and_changes_at_the_boundary() {
        let secret = Secret::new(b"12345678901234567890".to_vec());
        let at = |time| {
            Totp.generate(&secret, &Params::new(), OtpInput::Time(time))
                .unwrap()
        };
        assert_eq!(at(30), at(59));
        assert_ne!(at(59), at(60));
    }

    #[test]
    fn honours_a_custom_period() {
        let secret = Secret::new(b"12345678901234567890".to_vec());
        let params = Params::new().with("period", "60");
        let at = |time| {
            Totp.generate(&secret, &params, OtpInput::Time(time))
                .unwrap()
        };
        assert_eq!(at(60), at(119));
        assert_ne!(at(119), at(120));
    }

    #[test]
    fn rejects_counter_input() {
        let result = Totp.generate(
            &Secret::new(b"12345678901234567890".to_vec()),
            &Params::new(),
            OtpInput::Counter(1),
        );
        assert_eq!(result, Err(OtpError::WrongInput { expected: "a time" }));
    }

    #[test]
    fn rejects_a_zero_period() {
        let params = Params::new().with("period", "0");
        assert!(matches!(
            Totp.generate(
                &Secret::new(b"12345678901234567890".to_vec()),
                &params,
                OtpInput::Time(1)
            ),
            Err(OtpError::Params(_))
        ));
    }
}
