use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultError {
    #[error("not a vault file")]
    NotAVault,
    #[error("unsupported vault file version {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported algorithm id {0}")]
    UnsupportedAlgorithm(u8),
    #[error("invalid vault header: {0}")]
    InvalidHeader(&'static str),
    #[error("invalid key derivation parameters: {0}")]
    InvalidKdfParams(&'static str),
    #[error("wrong password or damaged header")]
    UnwrapFailed,
    #[error("the vault body is damaged")]
    BodyCorrupted,
    #[error("unsupported payload version {0}")]
    UnsupportedPayloadVersion(i64),
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}
