//! What a joined device remembers about its sync file (ADR-0018). It lives only inside the
//! encrypted local vault: never in a sync file, an export or another device's vault.

use std::fmt;

use super::sync_file::{MasterKey, SyncId};
use super::sync_secret::SyncSecret;

#[derive(PartialEq, Eq)]
pub struct SyncState {
    pub sync_id: SyncId,
    pub master_key: MasterKey,
    pub sync_secret: SyncSecret,
    /// Where this device finds the file: a path, or on Android a document address.
    pub location: String,
    /// The highest revision of the file this device has seen or written.
    pub highest_revision: u64,
}

impl fmt::Debug for SyncState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SyncState(<redacted>)")
    }
}
