//! Google Authenticator's "Export accounts" links: `otpauth-migration://offline?data=…`, where
//! `data` is base64 of a small protobuf message listing the accounts. Several links belong
//! together when the export was split into batches; each is read on its own here.
//!
//! The message is read by a minimal reader instead of a protobuf library: only four field
//! kinds occur, every length is checked against the input, and unknown fields are skipped.
//! Errors never repeat any content.

use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use base64::Engine;
use mfa_core::otp::{OtpRegistry, Params};
use mfa_core::secret::Secret;
use mfa_core::vault::NewEntry;
use percent_encoding::percent_decode_str;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::import::{MAX_IMPORT_BYTES, MAX_ITEMS};
use crate::otpauth::split_label;

const SCHEME: &str = "otpauth-migration://";
const MAX_LINK_BYTES: usize = MAX_IMPORT_BYTES;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationError {
    #[error("this is not an otpauth-migration:// link")]
    NotMigration,
    #[error("the link is longer than 5 MiB")]
    TooLong,
    #[error("the link has no data")]
    NoData,
    #[error("the link's data is damaged")]
    Malformed,
    #[error("the link holds more than {MAX_ITEMS} accounts")]
    TooManyAccounts,
}

/// The accounts a link yielded, plus a fixed-sentence reason for each one that was not usable.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationBatch {
    pub entries: Vec<NewEntry>,
    pub skipped: Vec<String>,
}

#[must_use]
pub fn is_migration_link(text: &str) -> bool {
    text.trim()
        .get(..SCHEME.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(SCHEME))
}

/// # Errors
/// See [`MigrationError`]; accounts that cannot be used are listed in
/// [`MigrationBatch::skipped`] and never fail the link as a whole.
pub fn parse_migration_link(
    link: &str,
    registry: &OtpRegistry,
) -> Result<MigrationBatch, MigrationError> {
    let link = link.trim();
    if link.len() > MAX_LINK_BYTES {
        return Err(MigrationError::TooLong);
    }
    if !is_migration_link(link) {
        return Err(MigrationError::NotMigration);
    }
    let payload = decode_data(data_parameter(link)?)?;
    let mut batch = MigrationBatch::default();
    let mut count = 0;
    for account in accounts(&payload)? {
        count += 1;
        if count > MAX_ITEMS {
            return Err(MigrationError::TooManyAccounts);
        }
        match account.and_then(|raw| raw.into_entry(registry)) {
            Ok(entry) => batch.entries.push(entry),
            Err(reason) => batch.skipped.push(reason.to_owned()),
        }
    }
    Ok(batch)
}

fn data_parameter(link: &str) -> Result<&str, MigrationError> {
    let query = link.split_once('?').map_or("", |(_, query)| query);
    let query = query.split('#').next().unwrap_or_default();
    query
        .split('&')
        .find_map(|part| part.strip_prefix("data="))
        .filter(|data| !data.is_empty())
        .ok_or(MigrationError::NoData)
}

/// Percent-decoded, and tolerant of the base64 variants seen in the wild. A raw `+` stays a
/// plus: it is a base64 digit here, not a space.
fn decode_data(data: &str) -> Result<Zeroizing<Vec<u8>>, MigrationError> {
    let text = percent_decode_str(data)
        .decode_utf8()
        .map_err(|_| MigrationError::Malformed)?;
    let text = text.trim_end_matches('=');
    [STANDARD_NO_PAD, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(text).ok())
        .map(Zeroizing::new)
        .ok_or(MigrationError::Malformed)
}

// -- protobuf: message Payload { repeated OtpParameters otp_parameters = 1; ... } ------------

type Reason = &'static str;

struct RawAccount {
    secret: Zeroizing<Vec<u8>>,
    name: String,
    issuer: String,
    algorithm: u64,
    digits: u64,
    otp_type: u64,
    counter: u64,
}

fn accounts(payload: &[u8]) -> Result<Vec<Result<RawAccount, Reason>>, MigrationError> {
    let mut reader = Reader::new(payload);
    let mut found = Vec::new();
    while let Some((field, value)) = reader.next_field()? {
        if let (1, Value::Bytes(message)) = (field, value) {
            found.push(read_account(message));
        }
    }
    Ok(found)
}

fn read_account(message: &[u8]) -> Result<RawAccount, Reason> {
    let mut account = RawAccount {
        secret: Zeroizing::new(Vec::new()),
        name: String::new(),
        issuer: String::new(),
        algorithm: 0,
        digits: 0,
        otp_type: 0,
        counter: 0,
    };
    let mut reader = Reader::new(message);
    while let Some((field, value)) = reader.next_field().map_err(|_| "the account is damaged")? {
        match (field, value) {
            (1, Value::Bytes(bytes)) => account.secret = Zeroizing::new(bytes.to_vec()),
            (2, Value::Bytes(bytes)) => account.name = text(bytes)?,
            (3, Value::Bytes(bytes)) => account.issuer = text(bytes)?,
            (4, Value::Varint(number)) => account.algorithm = number,
            (5, Value::Varint(number)) => account.digits = number,
            (6, Value::Varint(number)) => account.otp_type = number,
            (7, Value::Varint(number)) => account.counter = number,
            _ => {}
        }
    }
    Ok(account)
}

fn text(bytes: &[u8]) -> Result<String, Reason> {
    std::str::from_utf8(bytes)
        .map(|text| text.trim().to_owned())
        .map_err(|_| "the account name is not readable text")
}

impl RawAccount {
    fn into_entry(self, registry: &OtpRegistry) -> Result<NewEntry, Reason> {
        let otp_type = match self.otp_type {
            1 => "hotp",
            2 => "totp",
            _ => return Err("the account type is not supported"),
        };
        let mut params = Params::new();
        match self.algorithm {
            0 | 1 => {}
            2 => params = params.with("algorithm", "SHA256"),
            3 => params = params.with("algorithm", "SHA512"),
            _ => return Err("the hash algorithm is not supported"),
        }
        match self.digits {
            0 | 1 => {}
            2 => params = params.with("digits", "8"),
            _ => return Err("the number of digits is not supported"),
        }
        if otp_type == "hotp" {
            params = params.with("counter", &self.counter.to_string());
        }
        let (label_issuer, account) = split_label(&self.name);
        let issuer = if self.issuer.is_empty() {
            label_issuer
        } else {
            self.issuer
        };
        if issuer.is_empty() && account.is_empty() {
            return Err("the account has no name");
        }
        let new = NewEntry {
            otp_type: otp_type.to_owned(),
            issuer,
            account,
            secret: Secret::new(self.secret.to_vec()),
            params,
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        };
        new.validate(registry)
            .map_err(|_| "the account's settings are not valid")?;
        Ok(new)
    }
}

enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Other,
}

struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    /// The next field number and value, `None` at the end. Fixed-size fields are skipped.
    fn next_field(&mut self) -> Result<Option<(u64, Value<'a>)>, MigrationError> {
        if self.rest.is_empty() {
            return Ok(None);
        }
        let key = self.varint()?;
        let value = match key & 7 {
            0 => Value::Varint(self.varint()?),
            1 => self.skip(8)?,
            2 => {
                let length =
                    usize::try_from(self.varint()?).map_err(|_| MigrationError::Malformed)?;
                Value::Bytes(self.take(length)?)
            }
            5 => self.skip(4)?,
            _ => return Err(MigrationError::Malformed),
        };
        Ok(Some((key >> 3, value)))
    }

    fn varint(&mut self) -> Result<u64, MigrationError> {
        let mut value = 0_u64;
        for index in 0..10 {
            let byte = *self.rest.get(index).ok_or(MigrationError::Malformed)?;
            value |= u64::from(byte & 0x7F) << (7 * index);
            if byte & 0x80 == 0 {
                self.rest = &self.rest[index + 1..];
                return Ok(value);
            }
        }
        Err(MigrationError::Malformed)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], MigrationError> {
        let (taken, rest) = self
            .rest
            .split_at_checked(length)
            .ok_or(MigrationError::Malformed)?;
        self.rest = rest;
        Ok(taken)
    }

    fn skip(&mut self, length: usize) -> Result<Value<'a>, MigrationError> {
        self.take(length)?;
        Ok(Value::Other)
    }
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::{STANDARD, URL_SAFE};

    use super::*;

    fn registry() -> OtpRegistry {
        OtpRegistry::with_builtins()
    }

    fn varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }

    fn number(field: u64, value: u64) -> Vec<u8> {
        [varint(field << 3), varint(value)].concat()
    }

    fn bytes(field: u64, value: &[u8]) -> Vec<u8> {
        [
            varint(field << 3 | 2),
            varint(value.len() as u64),
            value.to_vec(),
        ]
        .concat()
    }

    struct Account {
        secret: &'static [u8],
        name: &'static str,
        issuer: &'static str,
        algorithm: u64,
        digits: u64,
        otp_type: u64,
        counter: u64,
    }

    impl Account {
        const fn totp(name: &'static str, issuer: &'static str) -> Self {
            Self {
                secret: b"12345678901234567890",
                name,
                issuer,
                algorithm: 1,
                digits: 1,
                otp_type: 2,
                counter: 0,
            }
        }

        fn encode(&self) -> Vec<u8> {
            [
                bytes(1, self.secret),
                bytes(2, self.name.as_bytes()),
                bytes(3, self.issuer.as_bytes()),
                number(4, self.algorithm),
                number(5, self.digits),
                number(6, self.otp_type),
                number(7, self.counter),
            ]
            .concat()
        }
    }

    fn payload(accounts: &[Account]) -> Vec<u8> {
        let mut out: Vec<u8> = accounts
            .iter()
            .flat_map(|a| bytes(1, &a.encode()))
            .collect();
        out.extend(number(2, 1));
        out.extend(number(3, 1));
        out.extend(number(4, 0));
        out.extend(number(5, 123_456));
        out
    }

    fn link_of(payload: &[u8]) -> String {
        let data = STANDARD.encode(payload);
        let escaped = data
            .replace('+', "%2B")
            .replace('/', "%2F")
            .replace('=', "%3D");
        format!("otpauth-migration://offline?data={escaped}")
    }

    fn parse(accounts: &[Account]) -> MigrationBatch {
        parse_migration_link(&link_of(&payload(accounts)), &registry()).unwrap()
    }

    #[test]
    fn accounts_are_read_with_name_issuer_and_defaults() {
        let batch = parse(&[Account::totp("alice@example.com", "Example")]);
        assert_eq!(batch.skipped, [] as [String; 0]);
        let entry = &batch.entries[0];
        assert_eq!(
            (
                entry.otp_type.as_str(),
                entry.issuer.as_str(),
                entry.account.as_str()
            ),
            ("totp", "Example", "alice@example.com")
        );
        assert_eq!(entry.secret.as_bytes(), b"12345678901234567890");
        assert_eq!(
            entry.params.iter().count(),
            0,
            "SHA-1 and six digits are the defaults"
        );
    }

    #[test]
    fn issuer_comes_from_the_name_when_the_field_is_empty_and_the_prefix_is_not_doubled() {
        let batch = parse(&[
            Account::totp("Acme:bob", ""),
            Account::totp("Example:carol", "Example"),
            Account::totp("dave", ""),
        ]);
        let names: Vec<_> = batch
            .entries
            .iter()
            .map(|e| (e.issuer.as_str(), e.account.as_str()))
            .collect();
        assert_eq!(names, [("Acme", "bob"), ("Example", "carol"), ("", "dave")]);
    }

    #[test]
    fn algorithm_digits_and_counter_are_carried_over() {
        let mut sha512 = Account::totp("a", "x");
        sha512.algorithm = 3;
        sha512.digits = 2;
        let mut sha256 = Account::totp("b", "x");
        sha256.algorithm = 2;
        let mut counter = Account::totp("c", "x");
        counter.otp_type = 1;
        counter.counter = 41;
        let batch = parse(&[sha512, sha256, counter]);
        let params: Vec<Vec<(&str, &str)>> = batch
            .entries
            .iter()
            .map(|e| e.params.iter().collect())
            .collect();
        assert_eq!(params[0], [("algorithm", "SHA512"), ("digits", "8")]);
        assert_eq!(params[1], [("algorithm", "SHA256")]);
        assert_eq!(params[2], [("counter", "41")]);
        assert_eq!(batch.entries[2].otp_type, "hotp");
    }

    #[test]
    fn unusable_accounts_are_skipped_with_a_reason_and_the_rest_are_kept() {
        let mut md5 = Account::totp("a", "x");
        md5.algorithm = 4;
        let mut unknown_type = Account::totp("b", "x");
        unknown_type.otp_type = 0;
        let mut no_secret = Account::totp("c", "x");
        no_secret.secret = b"";
        let nameless = Account::totp("", "");
        let batch = parse(&[
            md5,
            unknown_type,
            no_secret,
            nameless,
            Account::totp("good", "x"),
        ]);
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(
            batch.skipped,
            [
                "the hash algorithm is not supported",
                "the account type is not supported",
                "the account's settings are not valid",
                "the account has no name",
            ]
        );
    }

    #[test]
    fn the_data_may_be_url_safe_unpadded_or_have_a_raw_plus() {
        let message = payload(&[Account::totp("alice", "Example")]);
        let padded = STANDARD.encode(&message);
        let raw_plus = format!("otpauth-migration://offline?data={padded}");
        let url_safe = format!(
            "otpauth-migration://offline?data={}",
            URL_SAFE.encode(&message)
        );
        let no_padding = format!(
            "otpauth-migration://offline?data={}",
            STANDARD_NO_PAD.encode(&message)
        );
        for link in [raw_plus, url_safe, no_padding, link_of(&message)] {
            let batch = parse_migration_link(&link, &registry()).unwrap();
            assert_eq!(batch.entries.len(), 1, "{link}");
        }
    }

    #[test]
    fn unknown_fields_and_wire_types_are_skipped() {
        let mut account = Account::totp("alice", "Example").encode();
        account.extend(number(15, 9));
        account.extend([varint(14 << 3 | 5), vec![0; 4]].concat());
        account.extend([varint(13 << 3 | 1), vec![0; 8]].concat());
        let message = [bytes(1, &account), bytes(9, b"future")].concat();
        let batch = parse_migration_link(&link_of(&message), &registry()).unwrap();
        assert_eq!(batch.entries.len(), 1);
    }

    #[test]
    fn other_links_and_missing_data_are_refused() {
        let r = registry();
        assert_eq!(
            parse_migration_link("otpauth://totp/x?secret=ABC", &r),
            Err(MigrationError::NotMigration)
        );
        assert_eq!(
            parse_migration_link("otpauth-migration://offline", &r),
            Err(MigrationError::NoData)
        );
        assert_eq!(
            parse_migration_link("otpauth-migration://offline?data=", &r),
            Err(MigrationError::NoData)
        );
        assert_eq!(
            parse_migration_link("otpauth-migration://offline?data=%%%", &r),
            Err(MigrationError::Malformed)
        );
        assert!(is_migration_link("  OTPAUTH-MIGRATION://offline?data=x"));
        assert!(!is_migration_link("otpauth://totp/x"));
    }

    #[test]
    fn truncated_or_lying_messages_are_malformed_and_never_panic() {
        let whole = payload(&[Account::totp("alice", "Example")]);
        for cut in 0..whole.len() {
            let _ = parse_migration_link(&link_of(&whole[..cut]), &registry());
        }
        let lying_length = [varint(1 << 3 | 2), varint(1000), vec![1, 2, 3]].concat();
        assert_eq!(
            parse_migration_link(&link_of(&lying_length), &registry()),
            Err(MigrationError::Malformed)
        );
        let endless_varint = [varint(1 << 3), vec![0xFF; 11]].concat();
        assert_eq!(
            parse_migration_link(&link_of(&endless_varint), &registry()),
            Err(MigrationError::Malformed)
        );
        let bad_wire = varint(1 << 3 | 7);
        assert_eq!(
            parse_migration_link(&link_of(&bad_wire), &registry()),
            Err(MigrationError::Malformed)
        );
    }

    #[test]
    fn a_damaged_account_inside_good_framing_is_skipped_not_fatal() {
        let damaged = bytes(1, &bytes(2, &[0xFF, 0xFE]));
        let message = [damaged, bytes(1, &Account::totp("ok", "x").encode())].concat();
        let batch = parse_migration_link(&link_of(&message), &registry()).unwrap();
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(batch.skipped, ["the account name is not readable text"]);
    }

    #[test]
    fn too_many_accounts_are_refused() {
        let one = bytes(1, &Account::totp("a", "x").encode());
        let message: Vec<u8> = std::iter::repeat_n(one, MAX_ITEMS + 1).flatten().collect();
        assert_eq!(
            parse_migration_link(&link_of(&message), &registry()),
            Err(MigrationError::TooManyAccounts)
        );
    }

    #[test]
    fn errors_never_contain_account_content() {
        let all = [
            MigrationError::NotMigration,
            MigrationError::TooLong,
            MigrationError::NoData,
            MigrationError::Malformed,
            MigrationError::TooManyAccounts,
        ];
        for error in all {
            assert!(!error.to_string().contains("12345"));
        }
    }
}
