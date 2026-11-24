use mfa_core::otp::OtpRegistry;

use super::{
    decode_text, ImportBatch, ImportError, ImportedEntry, Importer, SkippedItem, MAX_ITEMS,
};
use crate::otpauth::parse_otpauth;

/// A text file with one `otpauth://` link per line: the `WinAuth` export, many other apps'
/// plain exports, and the plaintext export of this app. Blank lines and lines starting with
/// `#` are ignored; any other line that is not a usable link is reported and skipped.
pub struct LinkListImporter;

impl Importer for LinkListImporter {
    fn id(&self) -> &'static str {
        "otpauth-list"
    }

    fn label(&self) -> &'static str {
        "Text file of otpauth:// links (WinAuth and others)"
    }

    fn parse(&self, input: &[u8], registry: &OtpRegistry) -> Result<ImportBatch, ImportError> {
        let text = decode_text(input)?;
        let mut batch = ImportBatch::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if batch.entries.len() + batch.skipped.len() == MAX_ITEMS {
                return Err(ImportError::TooManyItems);
            }
            let position = index + 1;
            match parse_otpauth(line, registry) {
                Ok(definition) => batch.entries.push(ImportedEntry {
                    position,
                    definition,
                }),
                Err(error) => batch.skipped.push(SkippedItem {
                    position,
                    reason: error.to_string(),
                }),
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

    const SECRET_A: &str = "JBSWY3DPEHPK3PXP";
    const SECRET_B: &str = "MZXW6YTBOI======";

    fn parse(input: &str) -> Result<ImportBatch, ImportError> {
        LinkListImporter.parse(input.as_bytes(), &OtpRegistry::with_builtins())
    }

    fn accounts(batch: &ImportBatch) -> Vec<(&str, &str)> {
        batch
            .entries
            .iter()
            .map(|e| (e.definition.issuer.as_str(), e.definition.account.as_str()))
            .collect()
    }

    #[test]
    fn a_winauth_style_export_is_read_line_by_line() {
        let file = format!(
            "otpauth://totp/Example?secret={SECRET_A}&digits=6&icon=WinAuth\r\n\
             otpauth://totp/admin@host.example?secret={SECRET_B}&digits=6\r\n\
             otpauth://totp/Acme:bob?secret={SECRET_A}&issuer=Acme&digits=8\r\n"
        );
        let batch = parse(&file).unwrap();
        assert_eq!(
            accounts(&batch),
            [("", "Example"), ("", "admin@host.example"), ("Acme", "bob")]
        );
        assert_eq!(
            batch.entries.iter().map(|e| e.position).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(batch.skipped.len(), 0);
    }

    #[test]
    fn blank_lines_comments_and_a_byte_order_mark_are_ignored() {
        let file =
            format!("\u{feff}# exported\n\n   \notpauth://totp/a?secret={SECRET_A}\n# end\n");
        let batch = parse(&file).unwrap();
        assert_eq!(batch.entries.len(), 1);
        assert_eq!(batch.entries[0].position, 4);
    }

    #[test]
    fn bad_lines_are_reported_with_their_line_number_and_do_not_block_the_rest() {
        let file = format!(
            "otpauth://totp/good1?secret={SECRET_A}\n\
             https://example.com/not-a-link\n\
             otpauth://totp/nosecret\n\
             otpauth://motp/other?secret={SECRET_A}\n\
             otpauth://totp/good2?secret={SECRET_B}\n"
        );
        let batch = parse(&file).unwrap();
        assert_eq!(accounts(&batch), [("", "good1"), ("", "good2")]);
        let skipped: Vec<_> = batch
            .skipped
            .iter()
            .map(|s| (s.position, s.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                (2, "this is not an otpauth:// link"),
                (3, "the link has no secret key"),
                (4, "the account type `motp` is not supported"),
            ]
        );
    }

    #[test]
    fn skip_reasons_never_contain_the_secret() {
        let batch = parse("otpauth://totp/me?secret=ZZtopSECRETvalue123\n").unwrap();
        assert_eq!(batch.entries.len(), 0);
        assert!(!batch.skipped[0].reason.contains("ZZtopSECRETvalue123"));
    }

    #[test]
    fn utf16_files_from_windows_editors_work() {
        let text = format!("otpauth://totp/a?secret={SECRET_A}\r\n");
        let bytes: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let batch = LinkListImporter
            .parse(&bytes, &OtpRegistry::with_builtins())
            .unwrap();
        assert_eq!(batch.entries.len(), 1);
    }

    #[test]
    fn files_with_nothing_in_them_say_so() {
        for text in ["", "   \n\n", "# only a comment\n"] {
            assert_eq!(parse(text), Err(ImportError::NothingFound), "{text:?}");
        }
    }

    #[test]
    fn a_file_of_only_bad_lines_is_still_a_report_not_an_error() {
        let batch = parse("hello\nworld\n").unwrap();
        assert_eq!((batch.entries.len(), batch.skipped.len()), (0, 2));
    }

    #[test]
    fn binary_and_oversized_input_is_refused() {
        let registry = OtpRegistry::with_builtins();
        assert_eq!(
            LinkListImporter.parse(&[0xC3, 0x28, 0x00], &registry),
            Err(ImportError::NotText)
        );
        let many = format!("otpauth://totp/a?secret={SECRET_A}\n").repeat(MAX_ITEMS + 1);
        assert_eq!(
            LinkListImporter.parse(many.as_bytes(), &registry),
            Err(ImportError::TooManyItems)
        );
        let exactly = format!("otpauth://totp/a?secret={SECRET_A}\n").repeat(MAX_ITEMS);
        assert_eq!(
            LinkListImporter
                .parse(exactly.as_bytes(), &registry)
                .unwrap()
                .entries
                .len(),
            MAX_ITEMS
        );
    }

    #[test]
    fn the_same_account_twice_is_kept_for_the_planner_to_judge() {
        let line = format!("otpauth://totp/dup?secret={SECRET_A}\n");
        assert_eq!(parse(&line.repeat(2)).unwrap().entries.len(), 2);
    }
}
