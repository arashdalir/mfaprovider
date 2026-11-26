//! Importers: one isolated codec per source format behind one interface (ADR-0004).
//! An importer only reads; merging with the vault is [`crate::merge`]'s job, so a preview can
//! be shown before anything changes.

mod freeotp;
mod google_migration;
mod link_list;

pub use freeotp::FreeOtpImporter;
pub use google_migration::GoogleMigrationImporter;
pub use link_list::LinkListImporter;

use mfa_core::otp::OtpRegistry;
use mfa_core::vault::NewEntry;
use thiserror::Error;

pub const MAX_IMPORT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ITEMS: usize = 5000;

/// One account read from a file. `position` says where it was (the line number for text
/// files), so a problem can be traced back without repeating any content.
#[derive(Debug, PartialEq, Eq)]
pub struct ImportedEntry {
    pub position: usize,
    pub definition: NewEntry,
}

/// Something in the file that could not be turned into an account. `reason` is a fixed
/// sentence and never contains the item's content.
#[derive(Debug, PartialEq, Eq)]
pub struct SkippedItem {
    pub position: usize,
    pub reason: String,
}

/// Everything readable in a file, plus what was not. One bad item never hides the good ones.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportBatch {
    pub entries: Vec<ImportedEntry>,
    pub skipped: Vec<SkippedItem>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImportError {
    #[error("the file is larger than 5 MiB")]
    TooLarge,
    #[error("the file is not readable text")]
    NotText,
    #[error("the file holds more than {MAX_ITEMS} items")]
    TooManyItems,
    #[error("the file is not in the expected format")]
    NotThisFormat,
    #[error("no accounts were found in the file")]
    NothingFound,
}

pub trait Importer: Send + Sync {
    /// Stable identifier, used by callers to pick the format.
    fn id(&self) -> &'static str;

    /// Name shown to the user.
    fn label(&self) -> &'static str;

    /// # Errors
    /// If the file as a whole cannot be read; problems with single items are reported in
    /// [`ImportBatch::skipped`] instead.
    fn parse(&self, input: &[u8], registry: &OtpRegistry) -> Result<ImportBatch, ImportError>;
}

#[must_use]
pub fn all_importers() -> Vec<Box<dyn Importer>> {
    vec![
        Box::new(LinkListImporter),
        Box::new(GoogleMigrationImporter),
        Box::new(FreeOtpImporter),
    ]
}

#[must_use]
pub fn importer(id: &str) -> Option<Box<dyn Importer>> {
    all_importers()
        .into_iter()
        .find(|candidate| candidate.id() == id)
}

/// UTF-8, or UTF-16 with a byte-order mark (what Windows editors write as "Unicode"); a
/// leading byte-order mark is dropped.
pub(crate) fn decode_text(input: &[u8]) -> Result<String, ImportError> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    if let Some(rest) = input.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, u16::from_le_bytes);
    }
    if let Some(rest) = input.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, u16::from_be_bytes);
    }
    let input = input.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(input);
    String::from_utf8(input.to_vec()).map_err(|_| ImportError::NotText)
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> Result<String, ImportError> {
    let (pairs, leftover) = bytes.as_chunks::<2>();
    if !leftover.is_empty() {
        return Err(ImportError::NotText);
    }
    let units: Vec<u16> = pairs.iter().map(|pair| unit(*pair)).collect();
    String::from_utf16(&units).map_err(|_| ImportError::NotText)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_read_as_utf8_with_or_without_a_byte_order_mark() {
        assert_eq!(decode_text(b"abc").unwrap(), "abc");
        assert_eq!(decode_text(b"\xEF\xBB\xBFabc").unwrap(), "abc");
        assert_eq!(decode_text("café".as_bytes()).unwrap(), "café");
    }

    #[test]
    fn utf16_with_a_byte_order_mark_is_read_in_both_byte_orders() {
        let le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("ab é".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("ab é".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode_text(&le).unwrap(), "ab é");
        assert_eq!(decode_text(&be).unwrap(), "ab é");
    }

    #[test]
    fn broken_encodings_and_oversized_files_are_refused() {
        assert_eq!(decode_text(&[0xFF, 0xFE, 0x41]), Err(ImportError::NotText));
        assert_eq!(
            decode_text(&[0xFF, 0xFE, 0x00, 0xD8]),
            Err(ImportError::NotText),
            "a lone surrogate"
        );
        assert_eq!(decode_text(&[0xC3, 0x28]), Err(ImportError::NotText));
        assert_eq!(
            decode_text(&vec![b'a'; MAX_IMPORT_BYTES + 1]),
            Err(ImportError::TooLarge)
        );
    }

    #[test]
    fn importers_are_found_by_id() {
        assert!(importer("otpauth-list").is_some());
        assert!(importer("google-migration").is_some());
        assert!(importer("freeotp").is_some());
        assert!(importer("nonsense").is_none());
        let ids: Vec<_> = all_importers().iter().map(|i| i.id()).collect();
        assert_eq!(ids, ["otpauth-list", "google-migration", "freeotp"]);
    }
}
