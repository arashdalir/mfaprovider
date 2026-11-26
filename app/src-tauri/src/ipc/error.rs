use mfa_core::vault::{EntryError, SessionError, VaultError};
use mfa_formats::archive::ArchiveError;
use mfa_formats::otpauth::OtpauthError;
use mfa_formats::qr::QrImageError;
use serde::Serialize;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum IpcErrorCode {
    Locked,
    NoVault,
    AlreadyExists,
    WrongPassword,
    PasswordRequired,
    /// Biometrics has gone too long without the master password; type it.
    PasswordDue,
    BiometricUnavailable,
    BiometricNotEnabled,
    BiometricCancelled,
    BiometricFailed,
    /// The device dropped the biometric key (a new fingerprint, say); it is switched off.
    BiometricInvalidated,
    NotFound,
    InvalidInput,
    Unsupported,
    StorageFailed,
    CorruptVault,
}

/// The only error the UI ever sees. The message is present only for input problems and is
/// built from our own text; it never contains passwords, secrets or file contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: Option<String>,
}

impl IpcError {
    pub const fn new(code: IpcErrorCode) -> Self {
        Self {
            code,
            message: None,
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::InvalidInput,
            message: Some(message.into()),
        }
    }
}

impl From<SessionError> for IpcError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::Locked => Self::new(IpcErrorCode::Locked),
            SessionError::NoVault => Self::new(IpcErrorCode::NoVault),
            SessionError::AlreadyExists => Self::new(IpcErrorCode::AlreadyExists),
            SessionError::Entry(error) => Self::from(error),
            SessionError::Vault(VaultError::UnwrapFailed) => Self::new(IpcErrorCode::WrongPassword),
            SessionError::Vault(_) => Self::new(IpcErrorCode::CorruptVault),
            SessionError::Storage(_) => Self::new(IpcErrorCode::StorageFailed),
        }
    }
}

impl From<OtpauthError> for IpcError {
    /// These messages are fixed sentences that never repeat the secret.
    fn from(error: OtpauthError) -> Self {
        Self::invalid_input(error.to_string())
    }
}

impl From<ArchiveError> for IpcError {
    /// Fixed sentences; a missing password is a code of its own so the UI can ask for it.
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::PasswordRequired => Self::new(IpcErrorCode::PasswordRequired),
            other => Self::invalid_input(other.to_string()),
        }
    }
}

impl From<QrImageError> for IpcError {
    fn from(error: QrImageError) -> Self {
        Self::invalid_input(error.to_string())
    }
}

impl From<EntryError> for IpcError {
    fn from(error: EntryError) -> Self {
        match error {
            EntryError::NotFound(_) => Self::new(IpcErrorCode::NotFound),
            EntryError::UnknownType(_) => Self::new(IpcErrorCode::Unsupported),
            other => Self::invalid_input(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use mfa_core::vault::{EntryId, StorageError};

    use super::*;

    fn code_of(error: impl Into<IpcError>) -> IpcErrorCode {
        error.into().code
    }

    #[test]
    fn session_errors_map_to_stable_codes() {
        assert_eq!(code_of(SessionError::Locked), IpcErrorCode::Locked);
        assert_eq!(code_of(SessionError::NoVault), IpcErrorCode::NoVault);
        assert_eq!(
            code_of(SessionError::AlreadyExists),
            IpcErrorCode::AlreadyExists
        );
        assert_eq!(
            code_of(SessionError::Vault(VaultError::UnwrapFailed)),
            IpcErrorCode::WrongPassword
        );
        assert_eq!(
            code_of(SessionError::Vault(VaultError::BodyCorrupted)),
            IpcErrorCode::CorruptVault
        );
        assert_eq!(
            code_of(SessionError::Storage(StorageError::new("disk full"))),
            IpcErrorCode::StorageFailed
        );
    }

    #[test]
    fn entry_errors_map_by_kind() {
        let id = EntryId::from_bytes([0; 16]);
        assert_eq!(code_of(EntryError::NotFound(id)), IpcErrorCode::NotFound);
        assert_eq!(
            code_of(EntryError::UnknownType("x".into())),
            IpcErrorCode::Unsupported
        );
        assert_eq!(
            code_of(SessionError::Entry(EntryError::EmptySecret)),
            IpcErrorCode::InvalidInput
        );
    }

    #[test]
    fn link_and_image_failures_are_input_errors_with_their_own_sentence() {
        let link = IpcError::from(OtpauthError::MissingSecret);
        assert_eq!(link.code, IpcErrorCode::InvalidInput);
        assert_eq!(link.message.as_deref(), Some("the link has no secret key"));
        let image = IpcError::from(QrImageError::NoQrCode);
        assert_eq!(image.code, IpcErrorCode::InvalidInput);
        assert_eq!(
            image.message.as_deref(),
            Some("no QR code was found in the image")
        );
    }

    #[test]
    fn only_input_problems_carry_a_message() {
        let id = EntryId::from_bytes([0; 16]);
        assert_eq!(IpcError::from(EntryError::NotFound(id)).message, None);
        assert_eq!(
            IpcError::from(SessionError::Storage(StorageError::new(
                "/home/me/vault.mfa"
            )))
            .message,
            None
        );
        assert!(IpcError::from(EntryError::EmptySecret).message.is_some());
    }

    #[test]
    fn errors_serialise_with_snake_case_codes() {
        let json = serde_json::to_string(&IpcError::new(IpcErrorCode::WrongPassword)).unwrap();
        assert_eq!(json, r#"{"code":"wrong_password","message":null}"#);
    }
}
