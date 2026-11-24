//! Wire types between the Rust core and the UI (ADR-0012).
//!
//! Outgoing types carry display data and the current code, never a seed. A seed travels
//! in exactly two places: `secret_base32` in the `*Request` types (the user typing or
//! scanning it in) and `RevealedSecret` (the one command behind re-authentication).
//! `no_secrets` tests enforce this layout.

use std::collections::BTreeMap;

use mfa_core::otp::Params;
use mfa_core::secret::Secret;
use mfa_core::vault::{CodeOutcome, Entry, EntryChanges, EntryCode, EntryId, NewEntry, VaultData};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::error::IpcError;

#[derive(Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
}

#[derive(Debug, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum CodeState {
    Ready {
        text: String,
        /// Absent for counter-based codes, which change only when the user asks for the next.
        period_seconds: Option<u32>,
        seconds_remaining: Option<u32>,
    },
    /// No implementation for this entry's OTP type; the entry is kept untouched.
    Unsupported,
    Failed,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct EntryView {
    pub id: String,
    pub otp_type: String,
    pub issuer: String,
    pub account: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    #[ts(type = "number")]
    pub updated_at_ms: i64,
    pub code: CodeState,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct NewEntryRequest {
    pub otp_type: String,
    pub issuer: String,
    pub account: String,
    pub secret_base32: String,
    pub params: BTreeMap<String, String>,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
}

/// Fields to change; `null` leaves a field as it is. The icon is removed with `clear_icon`.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct EntryChangesRequest {
    pub issuer: Option<String>,
    pub account: Option<String>,
    pub secret_base32: Option<String>,
    pub params: Option<BTreeMap<String, String>>,
    pub icon: Option<String>,
    pub clear_icon: bool,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

/// The seed of one entry, returned only after the user re-enters the master password.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct RevealedSecret {
    pub secret_base32: String,
}

impl RevealedSecret {
    pub fn of(entry: &Entry) -> Self {
        Self {
            secret_base32: entry.secret.to_base32().to_string(),
        }
    }
}

pub fn entry_views(data: &VaultData, codes: Vec<EntryCode>) -> Vec<EntryView> {
    data.entries
        .iter()
        .zip(codes)
        .map(|(entry, code)| EntryView::new(entry, code.outcome))
        .collect()
}

impl EntryView {
    fn new(entry: &Entry, outcome: CodeOutcome) -> Self {
        Self {
            id: entry.id.to_string(),
            otp_type: entry.otp_type.clone(),
            issuer: entry.issuer.clone(),
            account: entry.account.clone(),
            icon: entry.icon.clone(),
            tags: entry.tags.clone(),
            notes: entry.notes.clone(),
            created_at_ms: entry.created_at,
            updated_at_ms: entry.updated_at,
            code: CodeState::from(outcome),
        }
    }
}

impl From<CodeOutcome> for CodeState {
    fn from(outcome: CodeOutcome) -> Self {
        match outcome {
            CodeOutcome::Ready(live) => Self::Ready {
                text: live.code.as_str().to_owned(),
                period_seconds: live.validity.map(|v| saturating_u32(v.period_seconds)),
                seconds_remaining: live.validity.map(|v| saturating_u32(v.seconds_remaining)),
            },
            CodeOutcome::Unsupported => Self::Unsupported,
            CodeOutcome::Failed(_) => Self::Failed,
        }
    }
}

fn saturating_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn parse_entry_id(text: &str) -> Result<EntryId, IpcError> {
    EntryId::parse(text).ok_or_else(|| IpcError::invalid_input("not a valid entry id"))
}

fn parse_secret(text: &str) -> Result<Secret, IpcError> {
    Secret::from_base32(text).map_err(|_| IpcError::invalid_input("the secret is not valid base32"))
}

fn to_params(map: BTreeMap<String, String>) -> Params {
    map.iter().fold(Params::new(), |params, (name, value)| {
        params.with(name, value)
    })
}

impl TryFrom<NewEntryRequest> for NewEntry {
    type Error = IpcError;

    fn try_from(request: NewEntryRequest) -> Result<Self, IpcError> {
        Ok(Self {
            otp_type: request.otp_type,
            issuer: request.issuer,
            account: request.account,
            secret: parse_secret(&request.secret_base32)?,
            params: to_params(request.params),
            icon: request.icon,
            tags: request.tags,
            notes: request.notes,
        })
    }
}

impl TryFrom<EntryChangesRequest> for EntryChanges {
    type Error = IpcError;

    fn try_from(request: EntryChangesRequest) -> Result<Self, IpcError> {
        let icon = if request.clear_icon {
            Some(None)
        } else {
            request.icon.map(Some)
        };
        Ok(Self {
            issuer: request.issuer,
            account: request.account,
            secret: request
                .secret_base32
                .as_deref()
                .map(parse_secret)
                .transpose()?,
            params: request.params.map(to_params),
            icon,
            tags: request.tags,
            notes: request.notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use mfa_core::otp::OtpRegistry;

    use super::*;
    use crate::ipc::error::IpcErrorCode;

    const SEED: &[u8] = b"SuperSecretSeed!XY12";

    fn sample() -> (VaultData, EntryId) {
        let registry = OtpRegistry::with_builtins();
        let mut data = VaultData::empty(0);
        let id = EntryId::from_bytes([7; 16]);
        let new = NewEntry {
            otp_type: "totp".into(),
            issuer: "Example".into(),
            account: "me@example.com".into(),
            secret: Secret::new(SEED.to_vec()),
            params: Params::new(),
            icon: Some("example".into()),
            tags: vec!["work".into()],
            notes: "a note".into(),
        };
        data.add_entry(id, new, &registry, 1_700_000_000_123)
            .unwrap();
        (data, id)
    }

    fn views_at(data: &VaultData, now_ms: i64) -> Vec<EntryView> {
        let codes = data.codes(&OtpRegistry::with_builtins(), now_ms);
        entry_views(data, codes)
    }

    #[test]
    fn a_view_carries_display_data_and_the_current_code() {
        let (data, id) = sample();
        let views = views_at(&data, 59_000);
        let view = &views[0];
        assert_eq!(view.id, id.to_string());
        assert_eq!(view.issuer, "Example");
        assert_eq!(view.tags, ["work"]);
        let CodeState::Ready {
            text,
            period_seconds,
            seconds_remaining,
        } = &view.code
        else {
            panic!("expected a code, got {:?}", view.code);
        };
        assert_eq!(text.len(), 6);
        assert_eq!((*period_seconds, *seconds_remaining), (Some(30), Some(1)));
    }

    #[test]
    fn unsupported_and_failed_entries_are_marked_without_detail() {
        assert!(matches!(
            CodeState::from(CodeOutcome::Unsupported),
            CodeState::Unsupported
        ));
        let failed = CodeOutcome::Failed(mfa_core::otp::OtpError::EmptySecret);
        assert!(matches!(CodeState::from(failed), CodeState::Failed));
    }

    #[test]
    fn timestamps_are_sent_as_json_numbers() {
        let (data, _) = sample();
        let json = serde_json::to_value(&views_at(&data, 0)[0]).unwrap();
        assert_eq!(
            json["created_at_ms"],
            serde_json::json!(1_700_000_000_123_i64)
        );
        assert!(json["updated_at_ms"].is_number());
    }

    #[test]
    fn code_state_is_tagged_by_state() {
        let (data, _) = sample();
        let json = serde_json::to_value(&views_at(&data, 0)[0]).unwrap();
        assert_eq!(json["code"]["state"], "ready");
        let unsupported = serde_json::to_value(CodeState::Unsupported).unwrap();
        assert_eq!(unsupported, serde_json::json!({ "state": "unsupported" }));
    }

    #[test]
    fn a_new_entry_request_becomes_a_core_entry_definition() {
        let request: NewEntryRequest = serde_json::from_value(serde_json::json!({
            "otp_type": "totp", "issuer": "I", "account": "a",
            "secret_base32": "MZXW6YTBOI", "params": { "digits": "8" },
            "icon": null, "tags": ["t"], "notes": ""
        }))
        .unwrap();
        let new = NewEntry::try_from(request).unwrap();
        assert_eq!(new.secret.as_bytes(), b"foobar");
        assert_eq!(new.params.get("digits"), Some("8"));
    }

    #[test]
    fn a_bad_secret_is_an_input_error_that_does_not_echo_it() {
        let request: NewEntryRequest = serde_json::from_value(serde_json::json!({
            "otp_type": "totp", "issuer": "I", "account": "a",
            "secret_base32": "not base32!!", "params": {},
            "icon": null, "tags": [], "notes": ""
        }))
        .unwrap();
        let error = NewEntry::try_from(request).unwrap_err();
        assert_eq!(error.code, IpcErrorCode::InvalidInput);
        assert!(!error.message.unwrap().contains("not base32!!"));
    }

    #[test]
    fn changes_translate_the_icon_cases() {
        let change = |icon: Option<&str>, clear_icon: bool| {
            EntryChanges::try_from(EntryChangesRequest {
                issuer: None,
                account: None,
                secret_base32: None,
                params: None,
                icon: icon.map(str::to_owned),
                clear_icon,
                tags: None,
                notes: None,
            })
            .unwrap()
            .icon
        };
        assert_eq!(change(None, false), None);
        assert_eq!(change(Some("i"), false), Some(Some("i".to_owned())));
        assert_eq!(change(Some("i"), true), Some(None));
        assert_eq!(change(None, true), Some(None));
    }

    #[test]
    fn entry_ids_are_validated() {
        let (_, id) = sample();
        assert_eq!(parse_entry_id(&id.to_string()).unwrap(), id);
        assert_eq!(
            parse_entry_id("nope").unwrap_err().code,
            IpcErrorCode::InvalidInput
        );
    }

    #[test]
    fn reveal_returns_the_seed_as_base32() {
        let (data, id) = sample();
        let revealed = RevealedSecret::of(data.entry(id).unwrap());
        assert_eq!(
            revealed.secret_base32,
            Secret::new(SEED.to_vec()).to_base32().as_str()
        );
    }
}
