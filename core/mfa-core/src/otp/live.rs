use super::otp_type::{Code, CodeTiming, OtpError, OtpInput, OtpType};
use super::params::Params;
use crate::secret::Secret;

/// A code ready to show: for time-based types also how long it stays valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveCode {
    pub code: Code,
    /// `None` for counter-based types, whose code only changes when the counter advances.
    pub validity: Option<Validity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Validity {
    pub period_seconds: u64,
    /// Whole seconds until the next code, from 1 to `period_seconds`.
    pub seconds_remaining: u64,
}

/// The code `otp_type` shows at `now_ms` (Unix milliseconds; a time before 1970 counts as 1970).
///
/// # Errors
/// If the parameters are invalid for the type or the secret is empty.
pub fn live_code(
    otp_type: &dyn OtpType,
    secret: &Secret,
    params: &Params,
    now_ms: i64,
) -> Result<LiveCode, OtpError> {
    match otp_type.timing(params)? {
        CodeTiming::Counter(counter) => Ok(LiveCode {
            code: otp_type.generate(secret, params, OtpInput::Counter(counter))?,
            validity: None,
        }),
        CodeTiming::Time { period_seconds } => {
            let seconds = u64::try_from(now_ms).unwrap_or(0) / 1000;
            Ok(LiveCode {
                code: otp_type.generate(secret, params, OtpInput::Time(seconds))?,
                validity: Some(Validity {
                    period_seconds,
                    seconds_remaining: period_seconds - seconds % period_seconds,
                }),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otp::OtpRegistry;

    const RFC_SEED: &[u8] = b"12345678901234567890";

    fn live(id: &str, params: &Params, now_ms: i64) -> LiveCode {
        let registry = OtpRegistry::with_builtins();
        live_code(
            registry.get(id).unwrap(),
            &Secret::new(RFC_SEED.to_vec()),
            params,
            now_ms,
        )
        .unwrap()
    }

    fn eight_digits() -> Params {
        Params::new().with("digits", "8")
    }

    #[test]
    fn totp_shows_the_rfc_code_and_the_seconds_left() {
        let shown = live("totp", &eight_digits(), 59_000);
        assert_eq!(shown.code.as_str(), "94287082");
        assert_eq!(
            shown.validity,
            Some(Validity {
                period_seconds: 30,
                seconds_remaining: 1
            })
        );
    }

    #[test]
    fn a_new_period_starts_with_the_full_period_remaining() {
        let shown = live("totp", &eight_digits(), 60_000);
        assert_eq!(shown.validity.unwrap().seconds_remaining, 30);
        assert_ne!(shown.code.as_str(), "94287082");
    }

    #[test]
    fn milliseconds_inside_a_second_do_not_change_the_answer() {
        let early = live("totp", &eight_digits(), 59_000);
        let late = live("totp", &eight_digits(), 59_999);
        assert_eq!(early, late);
    }

    #[test]
    fn the_period_parameter_is_honoured() {
        let params = Params::new().with("period", "60");
        let shown = live("totp", &params, 61_000);
        assert_eq!(
            shown.validity,
            Some(Validity {
                period_seconds: 60,
                seconds_remaining: 59
            })
        );
    }

    #[test]
    fn steam_codes_last_thirty_seconds() {
        let shown = live("steam", &Params::new(), 31_000);
        assert_eq!(shown.code.as_str(), "PV9M4");
        assert_eq!(shown.validity.unwrap().seconds_remaining, 29);
    }

    #[test]
    fn hotp_shows_the_stored_counter_and_never_expires() {
        let first = live("hotp", &Params::new(), 123_456_789);
        assert_eq!(first.code.as_str(), "755224");
        assert_eq!(first.validity, None);
        let second = live("hotp", &Params::new().with("counter", "1"), 0);
        assert_eq!(second.code.as_str(), "287082");
    }

    #[test]
    fn a_time_before_1970_counts_as_1970() {
        assert_eq!(
            live("totp", &eight_digits(), -5_000),
            live("totp", &eight_digits(), 0)
        );
    }

    #[test]
    fn invalid_parameters_and_empty_secrets_are_errors() {
        let registry = OtpRegistry::with_builtins();
        let totp = registry.get("totp").unwrap();
        let bad = Params::new().with("period", "0");
        assert!(live_code(totp, &Secret::new(RFC_SEED.to_vec()), &bad, 0).is_err());
        assert_eq!(
            live_code(totp, &Secret::new(Vec::new()), &Params::new(), 0),
            Err(OtpError::EmptySecret)
        );
    }
}
