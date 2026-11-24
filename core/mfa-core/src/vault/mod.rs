//! The encrypted vault: data model, payload and file container (ADR-0011).

mod container;
mod error;
mod kdf;
mod model;
mod payload;
mod rng;
#[cfg(test)]
mod test_support;

pub use container::{create, open, OpenedVault};
pub use error::VaultError;
pub use kdf::{tune_kdf_params, KdfParams};
pub use model::{Entry, EntryId, Extra, Tombstone, VaultData};
pub use rng::Rng;
