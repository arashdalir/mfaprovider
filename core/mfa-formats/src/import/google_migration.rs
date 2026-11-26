use mfa_core::otp::OtpRegistry;

use super::{
    decode_text, ImportBatch, ImportError, ImportedEntry, Importer, SkippedItem, MAX_ITEMS,
};
use crate::migration::parse_migration_link;

/// Google Authenticator's "Export accounts": one `otpauth-migration://` link per line (an
/// export split into several QR codes gives several links; put them all in the file).
/// Blank lines and lines starting with `#` are ignored.
pub struct GoogleMigrationImporter;

impl Importer for GoogleMigrationImporter {
    fn id(&self) -> &'static str {
        "google-migration"
    }

    fn label(&self) -> &'static str {
        "Google Authenticator export (otpauth-migration:// links)"
    }

    fn parse(&self, input: &[u8], registry: &OtpRegistry) -> Result<ImportBatch, ImportError> {
        let text = decode_text(input)?;
        let mut batch = ImportBatch::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let position = index + 1;
            match parse_migration_link(line, registry) {
                Ok(parsed) => {
                    batch
                        .entries
                        .extend(parsed.entries.into_iter().map(|definition| ImportedEntry {
                            position,
                            definition,
                        }));
                    batch.skipped.extend(
                        parsed
                            .skipped
                            .into_iter()
                            .map(|reason| SkippedItem { position, reason }),
                    );
                }
                Err(error) => batch.skipped.push(SkippedItem {
                    position,
                    reason: error.to_string(),
                }),
            }
            if batch.entries.len() + batch.skipped.len() > MAX_ITEMS {
                return Err(ImportError::TooManyItems);
            }
        }
        if batch.entries.is_empty() && batch.skipped.is_empty() {
            return Err(ImportError::NothingFound);
        }
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<ImportBatch, ImportError> {
        GoogleMigrationImporter.parse(input.as_bytes(), &OtpRegistry::with_builtins())
    }

    fn short(length: usize) -> u8 {
        u8::try_from(length).unwrap()
    }

    /// `message` is a hand-made protobuf: field 1 (an account) holding secret, name, type.
    fn link(accounts: &[(&str, &[u8])]) -> String {
        use base64::Engine;
        let mut message = Vec::new();
        for (name, secret) in accounts {
            let mut account = vec![0x0A, short(secret.len())];
            account.extend(*secret);
            account.extend([0x12, short(name.len())]);
            account.extend(name.as_bytes());
            account.extend([0x30, 0x02]);
            message.extend([0x0A, short(account.len())]);
            message.extend(account);
        }
        format!(
            "otpauth-migration://offline?data={}",
            base64::engine::general_purpose::URL_SAFE.encode(message)
        )
    }

    #[test]
    fn every_link_in_the_file_contributes_its_accounts_with_the_line_number() {
        let file = format!(
            "{}\r\n\r\n# second batch\r\n{}\r\n",
            link(&[("one", b"aaaaaaaaaa"), ("Acme:two", b"bbbbbbbbbb")]),
            link(&[("three", b"cccccccccc")])
        );
        let batch = parse(&file).unwrap();
        let found: Vec<_> = batch
            .entries
            .iter()
            .map(|e| {
                (
                    e.position,
                    e.definition.issuer.as_str(),
                    e.definition.account.as_str(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(1, "", "one"), (1, "Acme", "two"), (4, "", "three")]
        );
        assert_eq!(batch.skipped, []);
    }

    #[test]
    fn a_bad_line_is_reported_and_the_good_ones_are_kept() {
        let file = format!(
            "otpauth://totp/x?secret=ABC\n{}\notpauth-migration://offline?data=%%%\n",
            link(&[("ok", b"dddddddddd")])
        );
        let batch = parse(&file).unwrap();
        assert_eq!(batch.entries.len(), 1);
        let skipped: Vec<_> = batch
            .skipped
            .iter()
            .map(|s| (s.position, s.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                (1, "this is not an otpauth-migration:// link"),
                (3, "the link's data is damaged"),
            ]
        );
    }

    #[test]
    fn an_empty_file_has_nothing_to_import() {
        assert_eq!(
            parse("\n# nothing\n").unwrap_err(),
            ImportError::NothingFound
        );
    }
}
