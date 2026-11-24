use mfa_core::vault::{EntryError, SessionError, VaultError};
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
