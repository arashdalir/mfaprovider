//! Sync operations of an unlocked vault (ADR-0018). They work on the bytes of the sync file;
//! reading and writing it is the caller's job (a storage provider).
//!
//! Anything that creates a sync file with new keys, or joins one, is split in two:
//! `prepare_…` changes nothing, `commit_…` stores the result in the vault. None of this
//! counts as user activity, so syncing in the background never delays the idle lock.

use std::collections::HashSet;

use thiserror::Error;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::{SessionError, VaultSession};
use crate::vault::clock::Clock;
use crate::vault::kdf::KdfParams;
use crate::vault::merge::merge;
use crate::vault::model::{Entry, EntryId, VaultData};
use crate::vault::rng::{random_array, random_key, Rng};
use crate::vault::storage::Storage;
use crate::vault::sync_file::{
    open_with_master_key, open_with_password, MasterKey, OpenedSyncFile, SyncFileError, SyncHeader,
    SYNC_ID_LENGTH,
};
use crate::vault::sync_secret::SyncSecret;
use crate::vault::sync_state::SyncState;

pub const MIN_SYNC_PASSWORD_CHARS: usize = 12;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncError {
    #[error("sync is not turned on")]
    NotEnabled,
    #[error("a sync password needs at least {MIN_SYNC_PASSWORD_CHARS} characters")]
    PasswordTooShort,
    #[error("a new master key needs a new place for the sync file")]
    SameLocation,
    #[error("the sync file's revision number cannot grow any further")]
    RevisionExhausted,
    #[error(transparent)]
    File(#[from] SyncFileError),
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// What may be shown about sync; holds no secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncStatus {
    pub location: String,
    pub highest_revision: u64,
}

/// Bytes to write over the version of the sync file that was read. Report success with
/// [`VaultSession::record_upload`].
#[derive(Debug, PartialEq, Eq)]
pub struct Upload {
    pub file: Vec<u8>,
    pub revision: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SyncStep {
    /// `None` when the file already holds everything this device knows.
    pub upload: Option<Upload>,
    pub local_changed: bool,
    /// The file is older than one this device has seen: an old read or a restored copy.
    pub older_than_seen: bool,
}

/// A sync file with new keys that is not yet in use: sync being turned on, or a new master
/// key. Create the file, let the user save the recovery sheet, then commit.
#[derive(Debug)]
pub struct PendingSyncSetup {
    state: SyncState,
    file: Vec<u8>,
}

impl PendingSyncSetup {
    #[must_use]
    pub fn file(&self) -> &[u8] {
        &self.file
    }

    #[must_use]
    pub fn sync_secret(&self) -> &SyncSecret {
        &self.state.sync_secret
    }
}

/// A sync file opened for joining; nothing has changed yet.
#[derive(Debug)]
pub struct PendingJoin {
    state: SyncState,
    header: SyncHeader,
    remote: VaultData,
}

/// What joining would do to this device's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinPreview {
    /// Accounts the list gains.
    pub added: usize,
    /// Accounts this device has that the file already holds in identical form.
    pub duplicates: usize,
    /// Accounts this device has that the file records as deleted since.
    pub removed: usize,
}

impl<S: Storage, C: Clock, R: Rng> VaultSession<S, C, R> {
    /// # Errors
    /// Locked.
    pub fn sync_status(&mut self) -> Result<Option<SyncStatus>, SyncError> {
        Ok(self.data()?.sync.as_ref().map(|sync| SyncStatus {
            location: sync.location.clone(),
            highest_revision: sync.highest_revision,
        }))
    }

    /// Builds a sync file with a fresh master key and sync secret: to turn sync on or, when
    /// it already is on, to replace the master key. The latter needs a new location.
    ///
    /// # Errors
    /// [`SyncError::PasswordTooShort`], [`SyncError::SameLocation`], locked, or a failure to
    /// derive a key or encrypt.
    pub fn prepare_new_sync(
        &mut self,
        sync_password: &str,
        location: &str,
        kdf: KdfParams,
    ) -> Result<PendingSyncSetup, SyncError> {
        check_sync_password(sync_password)?;
        let current = self.data()?.sync.as_ref();
        if current.is_some_and(|sync| sync.location == location) {
            return Err(SyncError::SameLocation);
        }
        let sync_id = random_array::<SYNC_ID_LENGTH>(&mut self.rng);
        let state = SyncState {
            sync_id,
            master_key: random_key(&mut self.rng),
            sync_secret: SyncSecret::random(&mut self.rng),
            location: location.to_owned(),
            highest_revision: 1,
        };
        let header = SyncHeader::lock(
            sync_password,
            &state.sync_secret,
            sync_id,
            &state.master_key,
            kdf,
            &mut self.rng,
        )?;
        let file = self.seal_current(&header, &state.master_key, 1)?;
        Ok(PendingSyncSetup { state, file })
    }

    /// Takes a prepared sync file into use, once it has been written.
    ///
    /// # Errors
    /// Locked, or the vault cannot be saved.
    pub fn commit_new_sync(&mut self, pending: PendingSyncSetup) -> Result<(), SyncError> {
        self.change_for_sync(|data| data.sync = Some(pending.state))
    }

    /// Opens an existing sync file for this device to join; changes nothing.
    ///
    /// # Errors
    /// [`SyncFileError::UnwrapFailed`] for a wrong password or secret, any other
    /// [`SyncFileError`], or locked.
    pub fn prepare_join(
        &mut self,
        file: &[u8],
        secret: SyncSecret,
        sync_password: &str,
        location: &str,
    ) -> Result<PendingJoin, SyncError> {
        self.data()?;
        let (master_key, opened) = open_with_password(sync_password, &secret, file)?;
        Ok(PendingJoin {
            state: SyncState {
                sync_id: opened.header.sync_id(),
                master_key,
                sync_secret: secret,
                location: location.to_owned(),
                highest_revision: opened.revision,
            },
            header: opened.header,
            remote: opened.data,
        })
    }

    /// # Errors
    /// Locked.
    pub fn preview_join(&mut self, pending: &PendingJoin) -> Result<JoinPreview, SyncError> {
        Ok(preview(self.data()?, &pending.remote))
    }

    /// Combines this device's accounts with the file's and turns sync on. The device's own
    /// password and vault key stay as they are.
    ///
    /// # Errors
    /// Locked, the vault cannot be saved, or the upload cannot be built.
    pub fn commit_join(&mut self, pending: PendingJoin) -> Result<SyncStep, SyncError> {
        let PendingJoin {
            state,
            header,
            remote,
        } = pending;
        let (master_key, revision) = (state.master_key.clone(), state.highest_revision);
        let (mut local_changed, mut remote_changed) = (false, false);
        self.change_for_sync(|data| {
            let dropped = drop_duplicates_of(data, &remote);
            let merged = merge(take(data), remote);
            local_changed = merged.local_changed || dropped;
            remote_changed = merged.remote_changed;
            *data = merged.data;
            data.sync = Some(state);
        })?;
        let upload = if remote_changed {
            Some(self.upload(&header, &master_key, revision)?)
        } else {
            None
        };
        Ok(SyncStep {
            upload,
            local_changed,
            older_than_seen: false,
        })
    }

    /// One sync round: merges what was read into the vault, saves it, and says what to write.
    ///
    /// # Errors
    /// [`SyncError::NotEnabled`]; [`SyncFileError::OtherMasterKey`] or
    /// [`SyncFileError::Damaged`] (nothing changes, and the file must not be overwritten);
    /// locked; or the vault cannot be saved.
    pub fn sync(&mut self, file: &[u8]) -> Result<SyncStep, SyncError> {
        let round = self.absorb(file)?;
        let upload = if round.file_lacks_something {
            Some(self.upload(&round.header, &round.master_key, round.seen)?)
        } else {
            None
        };
        Ok(SyncStep {
            upload,
            local_changed: round.local_changed,
            older_than_seen: round.older_than_seen,
        })
    }

    /// Like [`VaultSession::sync`], but the upload locks the same master key and sync secret
    /// with a new password, and is returned even if nothing else changed.
    ///
    /// # Errors
    /// [`SyncError::PasswordTooShort`], or as [`VaultSession::sync`].
    pub fn change_sync_password(
        &mut self,
        file: &[u8],
        new_password: &str,
        kdf: KdfParams,
    ) -> Result<SyncStep, SyncError> {
        check_sync_password(new_password)?;
        let round = self.absorb(file)?;
        let header = self.new_header(new_password, kdf)?;
        Ok(SyncStep {
            upload: Some(self.upload(&header, &round.master_key, round.seen)?),
            local_changed: round.local_changed,
            older_than_seen: round.older_than_seen,
        })
    }

    /// A fresh file from this device's vault with the same master key and sync secret, to
    /// replace a damaged or missing one when the user asks for it. There is nothing to check
    /// the password against, so it simply becomes the sync password.
    ///
    /// # Errors
    /// [`SyncError::PasswordTooShort`], [`SyncError::NotEnabled`], locked, or a failure to
    /// derive a key or encrypt.
    pub fn replacement_sync_file(
        &mut self,
        sync_password: &str,
        kdf: KdfParams,
    ) -> Result<Upload, SyncError> {
        check_sync_password(sync_password)?;
        let state = self.sync_state()?;
        let (master_key, highest) = (state.master_key.clone(), state.highest_revision);
        let header = self.new_header(sync_password, kdf)?;
        self.upload(&header, &master_key, highest)
    }

    /// Call once an [`Upload`] has been written.
    ///
    /// # Errors
    /// [`SyncError::NotEnabled`], locked, or the vault cannot be saved.
    pub fn record_upload(&mut self, revision: u64) -> Result<(), SyncError> {
        if self.sync_state()?.highest_revision >= revision {
            return Ok(());
        }
        self.change_for_sync(|data| {
            if let Some(sync) = &mut data.sync {
                sync.highest_revision = revision;
            }
        })
    }

    /// Forgets the sync settings. The accounts stay, and so does the sync file.
    ///
    /// # Errors
    /// [`SyncError::NotEnabled`], locked, or the vault cannot be saved.
    pub fn disable_sync(&mut self) -> Result<(), SyncError> {
        self.sync_state()?;
        self.change_for_sync(|data| data.sync = None)
    }

    /// The sync secret, for adding a device or printing the recovery sheet. Like revealing an
    /// account's secret, it needs the device password again.
    ///
    /// # Errors
    /// A wrong device password, [`SyncError::NotEnabled`], or locked.
    pub fn reveal_sync_secret(&mut self, device_password: &str) -> Result<SyncSecret, SyncError> {
        self.verify_password(device_password)?;
        Ok(self.sync_state()?.sync_secret.clone())
    }

    fn sync_state(&mut self) -> Result<&SyncState, SyncError> {
        self.data()?.sync.as_ref().ok_or(SyncError::NotEnabled)
    }

    /// Merges the file into the vault and notes the revision seen; saves only if something
    /// changed.
    fn absorb(&mut self, file: &[u8]) -> Result<Round, SyncError> {
        let state = self.sync_state()?;
        let (master_key, highest) = (state.master_key.clone(), state.highest_revision);
        let OpenedSyncFile {
            header,
            revision,
            data: remote,
        } = open_with_master_key(&state.sync_id, &master_key, file)?;
        let seen = revision.max(highest);

        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        let merged = merge(take(&mut unlocked.data), remote);
        unlocked.data = merged.data;
        let mut must_save = merged.local_changed || unlocked.dirty;
        if let Some(sync) = &mut unlocked.data.sync {
            must_save |= sync.highest_revision != seen;
            sync.highest_revision = seen;
        }
        if must_save {
            self.save_now()?;
        }
        Ok(Round {
            header,
            master_key,
            seen,
            local_changed: merged.local_changed,
            file_lacks_something: merged.remote_changed,
            older_than_seen: revision < highest,
        })
    }

    fn change_for_sync(&mut self, change: impl FnOnce(&mut VaultData)) -> Result<(), SyncError> {
        self.enforce_idle_timeout();
        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        change(&mut unlocked.data);
        self.save_now()
    }

    fn save_now(&mut self) -> Result<(), SyncError> {
        let unlocked = self.unlocked.as_mut().ok_or(SessionError::Locked)?;
        unlocked.dirty = true;
        Ok(Self::persist(unlocked, &mut self.storage, &mut self.rng)?)
    }

    /// A new password slot for the current master key and sync secret.
    fn new_header(&mut self, password: &str, kdf: KdfParams) -> Result<SyncHeader, SyncError> {
        let state = self.sync_state()?;
        let (sync_id, master_key, secret) = (
            state.sync_id,
            state.master_key.clone(),
            state.sync_secret.clone(),
        );
        Ok(SyncHeader::lock(
            password,
            &secret,
            sync_id,
            &master_key,
            kdf,
            &mut self.rng,
        )?)
    }

    /// The vault's shared data under the revision after `seen`.
    fn upload(
        &mut self,
        header: &SyncHeader,
        master_key: &MasterKey,
        seen: u64,
    ) -> Result<Upload, SyncError> {
        let revision = seen.checked_add(1).ok_or(SyncError::RevisionExhausted)?;
        Ok(Upload {
            file: self.seal_current(header, master_key, revision)?,
            revision,
        })
    }

    fn seal_current(
        &mut self,
        header: &SyncHeader,
        master_key: &MasterKey,
        revision: u64,
    ) -> Result<Vec<u8>, SyncError> {
        let unlocked = self.unlocked.as_ref().ok_or(SessionError::Locked)?;
        Ok(header.seal(master_key, revision, &unlocked.data, &mut self.rng)?)
    }
}

/// What reading the file taught this device.
struct Round {
    header: SyncHeader,
    master_key: MasterKey,
    seen: u64,
    local_changed: bool,
    file_lacks_something: bool,
    older_than_seen: bool,
}

/// Counted after Unicode normalisation, the form the key is derived from.
fn check_sync_password(password: &str) -> Result<(), SyncError> {
    let normalised: Zeroizing<String> = Zeroizing::new(password.nfkc().collect());
    if normalised.chars().count() < MIN_SYNC_PASSWORD_CHARS {
        return Err(SyncError::PasswordTooShort);
    }
    Ok(())
}

fn take(data: &mut VaultData) -> VaultData {
    std::mem::replace(data, VaultData::empty(0))
}

fn same_account(a: &Entry, b: &Entry) -> bool {
    a.otp_type == b.otp_type
        && a.issuer == b.issuer
        && a.account == b.account
        && a.secret.as_bytes() == b.secret.as_bytes()
        && a.params == b.params
}

/// Local accounts that are identical to one in the file under another id. Each account in the
/// file stands in for at most one of them, and none that this device already has by id.
fn duplicate_ids(local: &VaultData, remote: &VaultData) -> HashSet<EntryId> {
    let known_to_the_file =
        |id: EntryId| remote.entry(id).is_some() || remote.tombstones.iter().any(|t| t.id == id);
    let mut stood_in = HashSet::new();
    let mut duplicates = HashSet::new();
    for entry in local.entries.iter().filter(|e| !known_to_the_file(e.id)) {
        let twin = remote.entries.iter().find(|candidate| {
            local.entry(candidate.id).is_none()
                && !stood_in.contains(&candidate.id)
                && same_account(entry, candidate)
        });
        if let Some(twin) = twin {
            stood_in.insert(twin.id);
            duplicates.insert(entry.id);
        }
    }
    duplicates
}

/// Removes them without a deletion marker: their ids were never in this file. Returns
/// whether there were any.
fn drop_duplicates_of(local: &mut VaultData, remote: &VaultData) -> bool {
    let duplicates = duplicate_ids(local, remote);
    local
        .entries
        .retain(|entry| !duplicates.contains(&entry.id));
    !duplicates.is_empty()
}

fn preview(local: &VaultData, remote: &VaultData) -> JoinPreview {
    let duplicates = duplicate_ids(local, remote).len();
    let deleted_in = |entry: &Entry, copy: &VaultData| {
        copy.tombstones
            .iter()
            .any(|t| t.id == entry.id && t.deleted_at >= entry.updated_at)
    };
    let new_here = remote
        .entries
        .iter()
        .filter(|entry| local.entry(entry.id).is_none() && !deleted_in(entry, local))
        .count();
    JoinPreview {
        // The twin of a dropped duplicate takes its place, so the list does not grow by it.
        added: new_here.saturating_sub(duplicates),
        duplicates,
        removed: local
            .entries
            .iter()
            .filter(|entry| deleted_in(entry, remote))
            .count(),
    }
}

#[cfg(test)]
#[path = "sync_ops_tests.rs"]
mod tests;
