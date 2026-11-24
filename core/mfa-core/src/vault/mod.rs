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
pub use container::{create, open, OpenedVault};
pub use entries::{EntryChanges, EntryError, NewEntry};
pub use error::VaultError;
pub use kdf::{tune_kdf_params, KdfParams};
pub use model::{Entry, EntryId, Extra, Tombstone, VaultData};
pub use rng::Rng;
pub use session::{LockPolicy, SessionError, SystemEvent, VaultSession};
pub use storage::{MemoryStorage, Storage, StorageError};
