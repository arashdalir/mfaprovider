//! `otpauth://` key URIs, as written by Google Authenticator and nearly every service:
//! `otpauth://TYPE/LABEL?secret=…&issuer=…&algorithm=…&digits=…&period=…&counter=…`.
//!
//! Conventions chosen here for the ambiguous parts: a repeated query parameter is rejected;
//! `+` in a value means a space; the `issuer` parameter wins over the label prefix; the host
//! `steam` means Steam Guard (a `totp` URI whose issuer is "Steam" stays TOTP); unknown
//! parameters are ignored. Errors never repeat the secret.

use mfa_core::otp::{OtpRegistry, Params};
use mfa_core::secret::Secret;
use mfa_core::vault::{Entry, EntryError, NewEntry};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use thiserror::Error;
use zeroize::Zeroizing;

pub const MAX_URI_BYTES: usize = 2048;
const SCHEME: &str = "otpauth://";
const MAX_TYPE_ECHO: usize = 32;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OtpauthError {
    #[error("the text is longer than {MAX_URI_BYTES} bytes")]
    TooLong,
    #[error("this is not an otpauth:// link")]
    NotOtpauth,
    #[error("the link is malformed")]
    Malformed,
    #[error("the account type `{0}` is not supported")]
    UnsupportedType(String),
    #[error("the link has no account name or issuer")]
    NoLabel,
    #[error("the link has no secret key")]
    MissingSecret,
    #[error("the secret key is not valid base32")]
    InvalidSecret,
    #[error("the link has no counter, which counter-based accounts need")]
    MissingCounter,
    #[error("the link repeats the parameter `{0}`")]
    DuplicateParameter(String),
    #[error(transparent)]
    Invalid(#[from] EntryError),
}

/// Parses `uri` into an entry definition that is already validated against `registry`.
///
/// # Errors
/// See [`OtpauthError`]; none of its messages contain the secret.
pub fn parse_otpauth(uri: &str, registry: &OtpRegistry) -> Result<NewEntry, OtpauthError> {
    let uri = uri.trim();
    if uri.len() > MAX_URI_BYTES {
        return Err(OtpauthError::TooLong);
    }
    let rest = strip_scheme(uri).ok_or(OtpauthError::NotOtpauth)?;
    let rest = rest.split('#').next().unwrap_or_default();
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (type_text, label) = path.split_once('/').ok_or(OtpauthError::Malformed)?;
    let otp_type = type_text.to_ascii_lowercase();
    let query = Query::parse(query)?;

    let (label_issuer, account) = split_label(&decode(label, false)?);
    let issuer = query
        .get("issuer")
        .map_or(label_issuer, |issuer| issuer.trim().to_owned());
    if issuer.is_empty() && account.is_empty() {
        return Err(OtpauthError::NoLabel);
    }

    let secret = Secret::from_base32(query.get("secret").ok_or(OtpauthError::MissingSecret)?)
        .map_err(|_| OtpauthError::InvalidSecret)?;
    let new = NewEntry {
        params: params_for(&otp_type, &query)?,
        otp_type,
        issuer,
        account,
        secret,
        icon: None,
        tags: Vec::new(),
        notes: String::new(),
    };
    check_type_is_known(&new, registry)?;
    new.validate(registry)?;
    Ok(new)
}

fn strip_scheme(uri: &str) -> Option<&str> {
    let (scheme, rest) = uri.split_at_checked(SCHEME.len())?;
    scheme.eq_ignore_ascii_case(SCHEME).then_some(rest)
}

fn check_type_is_known(new: &NewEntry, registry: &OtpRegistry) -> Result<(), OtpauthError> {
    if registry.get(&new.otp_type).is_none() {
        let shown: String = new.otp_type.chars().take(MAX_TYPE_ECHO).collect();
        return Err(OtpauthError::UnsupportedType(shown));
    }
    Ok(())
}

fn params_for(otp_type: &str, query: &Query) -> Result<Params, OtpauthError> {
    let names: &[&str] = match otp_type {
        "totp" => &["algorithm", "digits", "period"],
        "hotp" => &["algorithm", "digits", "counter"],
        _ => &[],
    };
    if otp_type == "hotp" && query.get("counter").is_none() {
        return Err(OtpauthError::MissingCounter);
    }
    Ok(names
        .iter()
        .fold(Params::new(), |params, name| match query.get(name) {
            Some(value) if *name == "algorithm" => params.with(name, &value.to_ascii_uppercase()),
            Some(value) => params.with(name, value.trim()),
            None => params,
        }))
}

const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

#[derive(Debug, Error, PartialEq, Eq)]
#[error("the account type `{0}` has no otpauth:// form")]
pub struct NoLinkForm(pub String);

/// The link an authenticator app scans for `entry`: the exact inverse of [`parse_otpauth`].
/// It contains the secret, so it is returned in a zeroising buffer.
///
/// # Errors
/// If the type is not one `otpauth://` can express (only `totp`, `hotp` and `steam`).
pub fn to_otpauth(entry: &Entry) -> Result<Zeroizing<String>, NoLinkForm> {
    let otp_type = entry.otp_type.as_str();
    if !matches!(otp_type, "totp" | "hotp" | "steam") {
        return Err(NoLinkForm(otp_type.chars().take(MAX_TYPE_ECHO).collect()));
    }
    let escape = |text: &str| utf8_percent_encode(text, UNRESERVED).to_string();
    let label = if entry.issuer.is_empty() {
        escape(&entry.account)
    } else {
        format!("{}:{}", escape(&entry.issuer), escape(&entry.account))
    };
    let mut link = Zeroizing::new(format!(
        "{SCHEME}{otp_type}/{label}?secret={}",
        entry.secret.to_base32().as_str()
    ));
    if !entry.issuer.is_empty() {
        link.push_str("&issuer=");
        link.push_str(&escape(&entry.issuer));
    }
    for (name, value) in entry.params.iter() {
        link.push('&');
        link.push_str(name);
        link.push('=');
        link.push_str(&escape(value));
    }
    if otp_type == "hotp" && entry.params.get("counter").is_none() {
        link.push_str("&counter=0");
    }
    Ok(link)
}

/// `issuer:account` or just `account`; whitespace around either part is dropped.
pub(crate) fn split_label(label: &str) -> (String, String) {
    match label.split_once(':') {
        Some((issuer, account)) => (issuer.trim().to_owned(), account.trim().to_owned()),
        None => (String::new(), label.trim().to_owned()),
    }
}

fn decode(text: &str, plus_is_space: bool) -> Result<String, OtpauthError> {
    let text = if plus_is_space {
        text.replace('+', " ")
    } else {
        text.to_owned()
    };
    percent_decode_str(&text)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| OtpauthError::Malformed)
}

struct Query(Vec<(String, String)>);

impl Query {
    fn parse(query: &str) -> Result<Self, OtpauthError> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for part in query.split('&').filter(|part| !part.is_empty()) {
            let (name, value) = part.split_once('=').unwrap_or((part, ""));
            let name = decode(name, true)?.to_ascii_lowercase();
            if pairs.iter().any(|(known, _)| *known == name) {
                return Err(OtpauthError::DuplicateParameter(
                    name.chars().take(MAX_TYPE_ECHO).collect(),
                ));
            }
            pairs.push((name, decode(value, true)?));
        }
        Ok(Self(pairs))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "JBSWY3DPEHPK3PXP";

    fn parse(uri: &str) -> Result<NewEntry, OtpauthError> {
        parse_otpauth(uri, &OtpRegistry::with_builtins())
    }

    fn ok(uri: &str) -> NewEntry {
        parse(uri).unwrap_or_else(|error| panic!("{uri}: {error}"))
    }

    fn params(entry: &NewEntry) -> Vec<(&str, &str)> {
        entry.params.iter().collect()
    }

    #[test]
    fn the_google_authenticator_example_parses() {
        let entry = ok(&format!(
            "otpauth://totp/Example:alice@google.com?secret={SECRET}&issuer=Example"
        ));
        assert_eq!(entry.otp_type, "totp");
        assert_eq!(entry.issuer, "Example");
        assert_eq!(entry.account, "alice@google.com");
        assert_eq!(entry.secret, Secret::from_base32(SECRET).unwrap());
        assert_eq!(params(&entry), []);
    }

    #[test]
    fn a_missing_issuer_leaves_it_empty() {
        let entry = ok(&format!("otpauth://totp/alice?secret={SECRET}"));
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("", "alice")
        );
    }

    #[test]
    fn the_issuer_can_come_from_the_label_or_the_parameter() {
        let from_label = ok(&format!("otpauth://totp/ACME:bob?secret={SECRET}"));
        assert_eq!(from_label.issuer, "ACME");
        let from_parameter = ok(&format!("otpauth://totp/bob?secret={SECRET}&issuer=ACME"));
        assert_eq!(from_parameter.issuer, "ACME");
        let both = ok(&format!(
            "otpauth://totp/Old:bob?secret={SECRET}&issuer=New"
        ));
        assert_eq!(both.issuer, "New", "the parameter wins");
    }

    #[test]
    fn an_issuer_alone_is_enough_of_a_label() {
        let entry = ok(&format!("otpauth://totp/?secret={SECRET}&issuer=ACME"));
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("ACME", "")
        );
    }

    #[test]
    fn percent_encoded_labels_are_decoded() {
        let entry = ok(&format!(
            "otpauth://totp/ACME%20Co:john.doe%40email.com?secret={SECRET}&issuer=ACME%20Co"
        ));
        assert_eq!(entry.issuer, "ACME Co");
        assert_eq!(entry.account, "john.doe@email.com");
    }

    #[test]
    fn an_encoded_colon_still_separates_issuer_from_account() {
        let entry = ok(&format!("otpauth://totp/ACME%3Ajohn?secret={SECRET}"));
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("ACME", "john")
        );
    }

    #[test]
    fn spaces_after_the_colon_and_unicode_are_handled() {
        let entry = ok(&format!(
            "otpauth://totp/Caf%C3%A9:%20%E2%9C%93%20me?secret={SECRET}"
        ));
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("Café", "✓ me")
        );
    }

    #[test]
    fn plus_in_a_value_means_a_space() {
        let entry = ok(&format!("otpauth://totp/me?secret={SECRET}&issuer=Foo+Bar"));
        assert_eq!(entry.issuer, "Foo Bar");
    }

    #[test]
    fn algorithm_digits_and_period_are_carried_over() {
        let entry = ok(&format!(
            "otpauth://totp/me?secret={SECRET}&algorithm=sha256&digits=8&period=60"
        ));
        assert_eq!(
            params(&entry),
            [("algorithm", "SHA256"), ("digits", "8"), ("period", "60")]
        );
    }

    #[test]
    fn hotp_needs_and_keeps_its_counter() {
        let entry = ok(&format!(
            "otpauth://hotp/me?secret={SECRET}&counter=18446744073709551615"
        ));
        assert_eq!(entry.otp_type, "hotp");
        assert_eq!(params(&entry), [("counter", "18446744073709551615")]);
        assert_eq!(
            parse(&format!("otpauth://hotp/me?secret={SECRET}")),
            Err(OtpauthError::MissingCounter)
        );
    }

    #[test]
    fn steam_is_its_own_type_and_ignores_totp_parameters() {
        let entry = ok(&format!(
            "otpauth://steam/Steam:me?secret={SECRET}&digits=5&period=30&algorithm=SHA1"
        ));
        assert_eq!(entry.otp_type, "steam");
        assert_eq!(params(&entry), []);
        let plain = ok(&format!(
            "otpauth://totp/Steam:me?secret={SECRET}&issuer=Steam"
        ));
        assert_eq!(plain.otp_type, "totp");
    }

    #[test]
    fn the_scheme_type_and_parameter_names_ignore_case() {
        let entry = ok(&format!(
            "OTPAUTH://TOTP/me?SECRET={SECRET}&Issuer=X&DIGITS=8"
        ));
        assert_eq!(entry.otp_type, "totp");
        assert_eq!(entry.issuer, "X");
        assert_eq!(params(&entry), [("digits", "8")]);
    }

    #[test]
    fn lenient_secrets_unknown_parameters_fragments_and_outer_whitespace_are_fine() {
        let entry =
            ok("  otpauth://totp/me?image=https%3A%2F%2Fx&secret=jbsw%20y3dp-ehpk3pxp&lock=true#top \n");
        assert_eq!(entry.secret, Secret::from_base32(SECRET).unwrap());
    }

    #[test]
    fn unsupported_types_are_reported_without_dumping_long_text() {
        assert_eq!(
            parse(&format!("otpauth://motp/me?secret={SECRET}")),
            Err(OtpauthError::UnsupportedType("motp".into()))
        );
        let long = "x".repeat(200);
        let Err(OtpauthError::UnsupportedType(shown)) =
            parse(&format!("otpauth://{long}/me?secret={SECRET}"))
        else {
            panic!("expected an unsupported type");
        };
        assert_eq!(shown.len(), MAX_TYPE_ECHO);
    }

    #[test]
    fn parameters_the_type_rejects_surface_the_core_error() {
        for query in [
            "digits=9",
            "digits=abc",
            "period=0",
            "algorithm=MD5",
            "period=-30",
        ] {
            let result = parse(&format!("otpauth://totp/me?secret={SECRET}&{query}"));
            assert!(
                matches!(result, Err(OtpauthError::Invalid(_))),
                "{query}: {result:?}"
            );
        }
    }

    #[test]
    fn secrets_that_are_missing_empty_or_not_base32_are_refused() {
        assert_eq!(parse("otpauth://totp/me"), Err(OtpauthError::MissingSecret));
        assert_eq!(
            parse("otpauth://totp/me?issuer=X"),
            Err(OtpauthError::MissingSecret)
        );
        assert_eq!(
            parse("otpauth://totp/me?secret="),
            Err(OtpauthError::InvalidSecret)
        );
        assert_eq!(
            parse("otpauth://totp/me?secret=not%20base32!"),
            Err(OtpauthError::InvalidSecret)
        );
        assert_eq!(
            parse("otpauth://totp/me?secret=ABC"),
            Err(OtpauthError::InvalidSecret)
        );
    }

    #[test]
    fn a_repeated_parameter_is_ambiguous_and_refused() {
        assert_eq!(
            parse(&format!(
                "otpauth://totp/me?secret={SECRET}&secret=MZXW6YTBOI"
            )),
            Err(OtpauthError::DuplicateParameter("secret".into()))
        );
        assert_eq!(
            parse(&format!(
                "otpauth://totp/me?secret={SECRET}&Digits=6&digits=8"
            )),
            Err(OtpauthError::DuplicateParameter("digits".into()))
        );
    }

    #[test]
    fn things_that_are_not_otpauth_links_are_refused() {
        for text in [
            "",
            "   ",
            "https://example.com/?secret=JBSWY3DP",
            "otpauth:/totp/me?secret=JBSWY3DP",
            "otpauth-migration://offline?data=AAAA",
            "JBSWY3DPEHPK3PXP",
            "otpa",
        ] {
            assert_eq!(parse(text), Err(OtpauthError::NotOtpauth), "{text:?}");
        }
    }

    #[test]
    fn malformed_links_are_refused() {
        for text in [
            "otpauth://totp",
            "otpauth://totp?secret=JBSWY3DP",
            "otpauth://totp/me%FF?secret=JBSWY3DP",
            "otpauth://totp/me?secret=JBSWY3DP&issuer=%E2%28",
        ] {
            assert_eq!(parse(text), Err(OtpauthError::Malformed), "{text:?}");
        }
    }

    #[test]
    fn a_link_without_any_name_is_refused() {
        assert_eq!(
            parse(&format!("otpauth://totp/?secret={SECRET}")),
            Err(OtpauthError::NoLabel)
        );
        assert_eq!(
            parse(&format!("otpauth://totp/%20:%20?secret={SECRET}")),
            Err(OtpauthError::NoLabel)
        );
    }

    #[test]
    fn oversized_input_is_refused_before_any_parsing() {
        let long = format!(
            "otpauth://totp/me?secret={SECRET}&x={}",
            "a".repeat(MAX_URI_BYTES)
        );
        assert_eq!(parse(&long), Err(OtpauthError::TooLong));
        let exactly = format!("otpauth://totp/me?secret={SECRET}&x=");
        let padded = format!("{exactly}{}", "a".repeat(MAX_URI_BYTES - exactly.len()));
        assert!(parse(&padded).is_ok());
    }

    #[test]
    fn multibyte_text_cannot_split_the_scheme_check_inside_a_character() {
        assert_eq!(parse("otpauth:/é/me"), Err(OtpauthError::NotOtpauth));
        assert_eq!(parse("ééééééééé"), Err(OtpauthError::NotOtpauth));
    }

    #[test]
    fn no_error_message_contains_the_secret() {
        const HOSTILE_SECRET: &str = "ZZtopSECRETvalue123";
        let attempts = [
            format!("otpauth://totp/me?secret={HOSTILE_SECRET}"),
            format!("otpauth://totp/me?secret={HOSTILE_SECRET}&secret={HOSTILE_SECRET}"),
            format!("otpauth://motp/me?secret={HOSTILE_SECRET}"),
            format!("otpauth://totp/me?secret={SECRET}&digits={HOSTILE_SECRET}"),
            format!("otpauth://hotp/me?secret={HOSTILE_SECRET}"),
            format!("otpauth://totp/%FF?secret={HOSTILE_SECRET}"),
        ];
        for attempt in attempts {
            let error = parse(&attempt).unwrap_err();
            let shown = format!("{error} {error:?}");
            assert!(!shown.contains(HOSTILE_SECRET), "{attempt}: {shown}");
        }
    }

    #[test]
    fn a_parsed_entry_never_prints_its_secret() {
        let entry = ok(&format!("otpauth://totp/me?secret={SECRET}"));
        assert!(!format!("{entry:?}").contains(SECRET));
    }

    mod link_building {
        use super::*;
        use mfa_core::vault::{EntryId, Extra};

        fn as_entry(new: NewEntry) -> Entry {
            Entry {
                id: EntryId::from_bytes([7; 16]),
                otp_type: new.otp_type,
                issuer: new.issuer,
                account: new.account,
                secret: new.secret,
                params: new.params,
                icon: None,
                tags: Vec::new(),
                notes: String::new(),
                created_at: 0,
                updated_at: 0,
                extra: Extra::new(),
            }
        }

        fn link_of(uri: &str) -> String {
            to_otpauth(&as_entry(ok(uri))).unwrap().to_string()
        }

        fn same_account_after_a_round_trip(uri: &str) {
            let original = ok(uri);
            let again = ok(&link_of(uri));
            assert_eq!(again, original, "{uri}");
        }

        #[test]
        fn what_is_built_parses_back_to_the_same_account() {
            for uri in [
                format!("otpauth://totp/Example:alice@google.com?secret={SECRET}&issuer=Example"),
                format!("otpauth://totp/bob?secret={SECRET}"),
                format!("otpauth://totp/Acme:x?secret={SECRET}&algorithm=SHA512&digits=8&period=60&issuer=Acme"),
                format!("otpauth://hotp/Acme:x?secret={SECRET}&counter=42&issuer=Acme"),
                format!("otpauth://steam/Steam:me?secret={SECRET}&issuer=Steam"),
                format!("otpauth://totp/Caf%C3%A9%20%26%20Co:j%C3%BCrgen%2Bone%40x.de?secret={SECRET}&issuer=Caf%C3%A9%20%26%20Co"),
            ] {
                same_account_after_a_round_trip(&uri);
            }
        }

        #[test]
        fn the_link_keeps_issuer_and_account_apart_even_with_colons_and_spaces() {
            let link = link_of(&format!(
                "otpauth://totp/My%20Co:a%3Ab?secret={SECRET}&issuer=My%20Co"
            ));
            assert!(
                link.starts_with("otpauth://totp/My%20Co:a%3Ab?secret="),
                "{link}"
            );
            assert!(link.ends_with("&issuer=My%20Co"), "{link}");
        }

        #[test]
        fn counter_accounts_always_carry_a_counter() {
            let mut entry = as_entry(ok(&format!("otpauth://hotp/x?secret={SECRET}&counter=0")));
            entry.params = Params::new();
            let link = to_otpauth(&entry).unwrap();
            assert!(link.ends_with("&counter=0"), "{}", link.as_str());
        }

        #[test]
        fn types_without_a_link_form_are_refused() {
            let mut entry = as_entry(ok(&format!("otpauth://totp/x?secret={SECRET}")));
            entry.otp_type = "mystery".into();
            assert_eq!(
                to_otpauth(&entry).unwrap_err(),
                NoLinkForm("mystery".into())
            );
        }
    }
}
