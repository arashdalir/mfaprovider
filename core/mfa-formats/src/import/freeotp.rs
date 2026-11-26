use mfa_core::otp::{OtpRegistry, Params};
use mfa_core::secret::Secret;
use mfa_core::vault::NewEntry;
use serde::Deserialize;
use zeroize::{Zeroize, Zeroizing};

use super::{
    decode_text, ImportBatch, ImportError, ImportedEntry, Importer, SkippedItem, MAX_ITEMS,
};
use crate::otpauth::split_label;

/// `FreeOTP` and `FreeOTP+` JSON backup: `{"tokens": [...], "tokenOrder": [...]}`. The secret of a
/// token is a list of signed bytes. A token's position is its index in `tokens`, counting
/// from 1; `tokenOrder` is ignored.
pub struct FreeOtpImporter;

impl Importer for FreeOtpImporter {
    fn id(&self) -> &'static str {
        "freeotp"
    }

    fn label(&self) -> &'static str {
        "FreeOTP backup (JSON)"
    }

    fn parse(&self, input: &[u8], registry: &OtpRegistry) -> Result<ImportBatch, ImportError> {
        let text = decode_text(input)?;
        let backup: Backup = serde_json::from_str(&text).map_err(|_| ImportError::NotThisFormat)?;
        if backup.tokens.len() > MAX_ITEMS {
            return Err(ImportError::TooManyItems);
        }
        let mut batch = ImportBatch::default();
        for (index, token) in backup.tokens.iter().enumerate() {
            let position = index + 1;
            match token.to_entry(registry) {
                Ok(definition) => batch.entries.push(ImportedEntry {
                    position,
                    definition,
                }),
                Err(reason) => batch.skipped.push(SkippedItem {
                    position,
                    reason: reason.to_owned(),
                }),
            }
        }
        if batch.entries.is_empty() && batch.skipped.is_empty() {
            return Err(ImportError::NothingFound);
        }
        Ok(batch)
    }
}

#[derive(Deserialize)]
struct Backup {
    tokens: Vec<Token>,
}

#[derive(Deserialize)]
struct Token {
    #[serde(default)]
    algo: String,
    #[serde(default)]
    counter: u64,
    #[serde(default)]
    digits: u32,
    #[serde(default, rename = "issuerExt")]
    issuer: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    period: u32,
    #[serde(default)]
    secret: Vec<i8>,
    #[serde(default, rename = "type")]
    otp_type: String,
}

impl Drop for Token {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

impl Token {
    fn to_entry(&self, registry: &OtpRegistry) -> Result<NewEntry, &'static str> {
        let otp_type = match self.otp_type.to_ascii_uppercase().as_str() {
            "TOTP" => "totp",
            "HOTP" => "hotp",
            _ => return Err("the account type is not supported"),
        };
        let (label_issuer, account) = split_label(&self.label);
        let issuer = if self.issuer.is_empty() {
            label_issuer
        } else {
            self.issuer.clone()
        };
        if issuer.is_empty() && account.is_empty() {
            return Err("the account has no name");
        }
        let entry = NewEntry {
            otp_type: otp_type.to_owned(),
            issuer,
            account,
            secret: Secret::new(self.secret_bytes().to_vec()),
            params: self.params(otp_type)?,
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        };
        entry
            .validate(registry)
            .map_err(|_| "the account's settings are not valid")?;
        Ok(entry)
    }

    fn secret_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(
            self.secret
                .iter()
                .map(|&byte| byte.cast_unsigned())
                .collect(),
        )
    }

    fn params(&self, otp_type: &str) -> Result<Params, &'static str> {
        let mut params = Params::new();
        match self.algo.to_ascii_uppercase().as_str() {
            "" | "SHA1" => {}
            "SHA256" => params = params.with("algorithm", "SHA256"),
            "SHA512" => params = params.with("algorithm", "SHA512"),
            _ => return Err("the hash algorithm is not supported"),
        }
        if self.digits != 0 && self.digits != 6 {
            params = params.with("digits", &self.digits.to_string());
        }
        if otp_type == "hotp" {
            params = params.with("counter", &self.counter.to_string());
        } else if self.period != 0 && self.period != 30 {
            params = params.with("period", &self.period.to_string());
        }
        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<ImportBatch, ImportError> {
        FreeOtpImporter.parse(input.as_bytes(), &OtpRegistry::with_builtins())
    }

    const SECRET: &str = "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]";

    fn token(extra: &str) -> String {
        format!(
            r#"{{"algo":"SHA1","counter":0,"digits":6,"issuerExt":"","label":"alice","period":30,"secret":{SECRET},"type":"TOTP"{extra}}}"#
        )
    }

    fn backup(tokens: &[String]) -> String {
        format!(r#"{{"tokenOrder":["x"],"tokens":[{}]}}"#, tokens.join(","))
    }

    fn params(entry: &ImportedEntry) -> Vec<(&str, &str)> {
        entry.definition.params.iter().collect()
    }

    #[test]
    fn a_token_becomes_an_account_with_its_secret_bytes() {
        let batch = parse(&backup(&[
            token("").replace(r#""issuerExt":"""#, r#""issuerExt":"Acme""#)
        ]))
        .unwrap();
        let entry = &batch.entries[0];
        assert_eq!(
            (
                entry.position,
                entry.definition.issuer.as_str(),
                entry.definition.account.as_str()
            ),
            (1, "Acme", "alice")
        );
        assert_eq!(entry.definition.otp_type, "totp");
        assert_eq!(
            entry.definition.secret.as_bytes(),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        assert_eq!(params(entry), []);
    }

    #[test]
    fn signed_bytes_are_read_as_their_unsigned_values() {
        let negative = token("").replace(SECRET, "[-1, -128, 127, 0]");
        let batch = parse(&backup(&[negative])).unwrap();
        assert_eq!(
            batch.entries[0].definition.secret.as_bytes(),
            [255, 128, 127, 0]
        );
    }

    #[test]
    fn the_issuer_comes_from_the_label_when_the_field_is_empty() {
        let labelled = token("").replace("alice", "Acme:alice");
        let batch = parse(&backup(&[labelled])).unwrap();
        let definition = &batch.entries[0].definition;
        assert_eq!(
            (definition.issuer.as_str(), definition.account.as_str()),
            ("Acme", "alice")
        );
    }

    #[test]
    fn other_settings_are_kept() {
        let custom = token("")
            .replace("SHA1", "SHA256")
            .replace("\"digits\":6", "\"digits\":8")
            .replace("\"period\":30", "\"period\":60");
        let batch = parse(&backup(&[custom])).unwrap();
        assert_eq!(
            params(&batch.entries[0]),
            [("algorithm", "SHA256"), ("digits", "8"), ("period", "60")]
        );
    }

    #[test]
    fn counter_based_tokens_keep_their_counter() {
        let counter = token("")
            .replace("TOTP", "HOTP")
            .replace("\"counter\":0", "\"counter\":42");
        let batch = parse(&backup(&[counter])).unwrap();
        assert_eq!(batch.entries[0].definition.otp_type, "hotp");
        assert_eq!(params(&batch.entries[0]), [("counter", "42")]);
    }

    #[test]
    fn unusable_tokens_are_reported_by_position_and_the_rest_is_kept() {
        let md5 = token("").replace("SHA1", "MD5");
        let steam = token("").replace("TOTP", "STEAM");
        let nameless = token("").replace("alice", "");
        let empty_secret = token("").replace(SECRET, "[]");
        let batch = parse(&backup(&[md5, token(""), steam, nameless, empty_secret])).unwrap();
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(batch.entries[0].position, 2);
        let skipped: Vec<_> = batch
            .skipped
            .iter()
            .map(|s| (s.position, s.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                (1, "the hash algorithm is not supported"),
                (3, "the account type is not supported"),
                (4, "the account has no name"),
                (5, "the account's settings are not valid"),
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_a_backup_is_refused_without_echoing_it() {
        for text in ["not json", "[]", r#"{"tokens":"x"}"#, r#"{"other":1}"#] {
            assert_eq!(
                parse(text).unwrap_err(),
                ImportError::NotThisFormat,
                "{text}"
            );
        }
    }

    #[test]
    fn a_secret_that_is_not_bytes_makes_the_file_unreadable() {
        let broken = token("").replace(SECRET, "[1, 300]");
        assert_eq!(
            parse(&backup(&[broken])).unwrap_err(),
            ImportError::NotThisFormat
        );
    }

    #[test]
    fn a_backup_without_tokens_has_nothing_to_import() {
        assert_eq!(
            parse(r#"{"tokens":[]}"#).unwrap_err(),
            ImportError::NothingFound
        );
    }

    #[test]
    fn too_many_tokens_are_refused() {
        let many = vec![token(""); MAX_ITEMS + 1];
        assert_eq!(
            parse(&backup(&many)).unwrap_err(),
            ImportError::TooManyItems
        );
    }
}
