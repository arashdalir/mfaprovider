//! The current code of every entry, for the list the UI shows.

use super::model::{EntryId, VaultData};
use crate::otp::{live_code, LiveCode, OtpError, OtpRegistry};

#[derive(Debug, PartialEq, Eq)]
pub enum CodeOutcome {
    Ready(LiveCode),
    /// No implementation for the entry's OTP type; the entry is kept untouched (ADR-0004).
    Unsupported,
    Failed(OtpError),
}

#[derive(Debug, PartialEq, Eq)]
pub struct EntryCode {
    pub id: EntryId,
    pub outcome: CodeOutcome,
}

impl VaultData {
    /// One result per entry, in list order. A broken entry never hides the others.
    #[must_use]
    pub fn codes(&self, registry: &OtpRegistry, now_ms: i64) -> Vec<EntryCode> {
        self.entries
            .iter()
            .map(|entry| {
                let outcome = match registry.get(&entry.otp_type) {
                    None => CodeOutcome::Unsupported,
                    Some(otp_type) => {
                        match live_code(otp_type, &entry.secret, &entry.params, now_ms) {
                            Ok(code) => CodeOutcome::Ready(code),
                            Err(error) => CodeOutcome::Failed(error),
                        }
                    }
                };
                EntryCode {
                    id: entry.id,
                    outcome,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{id, sample_vault};
    use super::*;
    use crate::otp::Params;
    use crate::secret::Secret;

    fn codes_at(data: &VaultData, now_ms: i64) -> Vec<EntryCode> {
        data.codes(&OtpRegistry::with_builtins(), now_ms)
    }

    #[test]
    fn supported_entries_get_codes_and_unsupported_ones_are_flagged() {
        let codes = codes_at(&sample_vault(), 59_000);
        assert_eq!(codes.len(), 2);
        assert_eq!((codes[0].id, codes[1].id), (id(1), id(2)));
        assert!(
            matches!(&codes[0].outcome, CodeOutcome::Ready(live) if live.code.as_str().len() == 8)
        );
        assert_eq!(codes[1].outcome, CodeOutcome::Unsupported);
    }

    #[test]
    fn a_broken_entry_does_not_hide_the_others() {
        let mut data = sample_vault();
        data.entries[0].secret = Secret::new(Vec::new());
        data.entries[1].otp_type = "totp".into();
        data.entries[1].params = Params::new();
        let codes = codes_at(&data, 0);
        assert_eq!(codes[0].outcome, CodeOutcome::Failed(OtpError::EmptySecret));
        assert!(matches!(codes[1].outcome, CodeOutcome::Ready(_)));
    }

    #[test]
    fn an_empty_vault_has_no_codes() {
        assert_eq!(codes_at(&VaultData::empty(0), 0).len(), 0);
    }
}
