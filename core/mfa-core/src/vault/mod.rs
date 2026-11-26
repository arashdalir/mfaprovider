//! The encrypted vault: data model, payload and file container (ADR-0011).

mod clock;
mod codes;
mod container;
mod entries;
mod error;
mod kdf;
mod model;
mod payload;
mod rng;
mod session;
mod storage;
#[cfg(test)]
mod test_support;

pub use clock::Clock;
pub use codes::{CodeOutcome, EntryCode};
pub use container::{create, open, open_with_key, OpenedVault, VAULT_KEY_LENGTH};
pub use entries::{EntryChanges, EntryError, ImportOp, NewEntry};
pub use error::VaultError;
pub use kdf::{run_kdf_benchmark, tune_kdf_params, KdfParams};
pub use model::{Entry, EntryId, Extra, Tombstone, VaultData};
pub use rng::Rng;
pub use session::{LockPolicy, SessionError, SystemEvent, VaultSession};
pub use storage::{MemoryStorage, Storage, StorageError};
