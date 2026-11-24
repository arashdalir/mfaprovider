use std::sync::{Arc, Mutex, PoisonError};

use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("storage failure: {0}")]
pub struct StorageError(String);

impl StorageError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Where the vault file bytes live. The core never touches a filesystem itself.
pub trait Storage {
    /// # Errors
    /// If the storage cannot be read. A missing vault is `Ok(None)`, not an error.
    fn read(&self) -> Result<Option<Vec<u8>>, StorageError>;

    /// Replaces the stored bytes. Must be **atomic**: after a crash or error the storage
    /// holds either the complete old bytes or the complete new bytes, never a mixture.
    ///
    /// # Errors
    /// If the bytes could not be stored; the previous bytes must then still be intact.
    fn write(&mut self, bytes: &[u8]) -> Result<(), StorageError>;
}

#[derive(Default)]
struct MemoryState {
    bytes: Option<Vec<u8>>,
    failing: bool,
}

/// In-memory [`Storage`]. Clones share the same bytes, so a test can "restart" by
/// building a second session over a clone. Writes can be made to fail.
#[derive(Clone, Default)]
pub struct MemoryStorage(Arc<Mutex<MemoryState>>);

impl MemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// While `true`, every write fails and leaves the stored bytes untouched.
    pub fn set_failing(&self, failing: bool) {
        self.state().failing = failing;
    }

    #[must_use]
    pub fn bytes(&self) -> Option<Vec<u8>> {
        self.state().bytes.clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, MemoryState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Storage for MemoryStorage {
    fn read(&self) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.bytes())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        let mut state = self.state();
        if state.failing {
            return Err(StorageError::new("simulated write failure"));
        }
        state.bytes = Some(bytes.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_returns_what_was_written() {
        let mut storage = MemoryStorage::new();
        assert_eq!(storage.read().unwrap(), None);
        storage.write(b"one").unwrap();
        storage.write(b"two").unwrap();
        assert_eq!(storage.read().unwrap(), Some(b"two".to_vec()));
    }

    #[test]
    fn clones_share_the_same_bytes() {
        let mut first = MemoryStorage::new();
        let second = first.clone();
        first.write(b"shared").unwrap();
        assert_eq!(second.read().unwrap(), Some(b"shared".to_vec()));
    }

    #[test]
    fn a_failing_write_leaves_the_old_bytes_in_place() {
        let mut storage = MemoryStorage::new();
        storage.write(b"old").unwrap();
        storage.set_failing(true);
        assert!(storage.write(b"new").is_err());
        assert_eq!(storage.read().unwrap(), Some(b"old".to_vec()));
        storage.set_failing(false);
        storage.write(b"new").unwrap();
        assert_eq!(storage.read().unwrap(), Some(b"new".to_vec()));
    }
}
