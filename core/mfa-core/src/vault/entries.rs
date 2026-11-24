//! Adding, editing, deleting, reordering and searching entries (TASK-0007).
//!
//! Entry order is the list order of [`VaultData::entries`] (ADR-0011); there is no sort key.
//! Every operation validates before it changes anything, so an error leaves the data untouched.

use thiserror::Error;

use super::model::{Entry, EntryId, Extra, Tombstone, VaultData};
use crate::otp::{CodeTiming, OtpRegistry, ParamError, Params};
use crate::secret::Secret;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntryError {
    #[error("no entry with id {0}")]
    NotFound(EntryId),
    #[error("an entry or deletion marker with id {0} already exists")]
    DuplicateId(EntryId),
    #[error("unknown OTP type `{0}`")]
    UnknownType(String),
    #[error(transparent)]
    InvalidParams(#[from] ParamError),
    #[error("the secret is empty")]
    EmptySecret,
    #[error("position {position} is outside a list of {len} entries")]
    PositionOutOfRange { position: usize, len: usize },
    #[error("entry {0} is not counter-based")]
    NotCounterBased(EntryId),
    #[error("the counter cannot be advanced any further")]
    CounterOverflow,
}

/// What the user supplies when adding an account.
#[derive(Debug, PartialEq, Eq)]
pub struct NewEntry {
    pub otp_type: String,
    pub issuer: String,
    pub account: String,
    pub secret: Secret,
    pub params: Params,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
}

/// One step of an import, applied together with the others or not at all.
#[derive(Debug, PartialEq, Eq)]
pub enum ImportOp {
    /// Adds a new entry under `id`.
    Add { id: EntryId, new: NewEntry },
    /// Overwrites an existing entry's type, name, secret and parameters; its id, position,
    /// tags, notes and icon stay.
    Replace { id: EntryId, with: NewEntry },
}

/// Fields to change; `None` leaves a field as it is. The OTP type cannot change: delete the
/// entry and add a new one instead.
#[derive(Debug, Default)]
pub struct EntryChanges {
    pub issuer: Option<String>,
    pub account: Option<String>,
    pub secret: Option<Secret>,
    pub params: Option<Params>,
    /// `Some(None)` removes the icon.
    pub icon: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
}

impl NewEntry {
    /// Checks the secret and parameters the way adding does, without adding anything.
    ///
    /// # Errors
    /// [`EntryError::EmptySecret`], [`EntryError::UnknownType`] or [`EntryError::InvalidParams`].
    pub fn validate(&self, registry: &OtpRegistry) -> Result<(), EntryError> {
        validate_secret(&self.secret)?;
        validate_params(registry, &self.otp_type, &self.params)
    }
}

impl VaultData {
    /// # Errors
    /// [`EntryError::DuplicateId`] if `id` belongs to an entry or a deletion marker;
    /// [`EntryError::UnknownType`], [`EntryError::InvalidParams`] or [`EntryError::EmptySecret`]
    /// for an invalid definition.
    pub fn add_entry(
        &mut self,
        id: EntryId,
        new: NewEntry,
        registry: &OtpRegistry,
        now_ms: i64,
    ) -> Result<(), EntryError> {
        if self.position_of(id).is_ok() || self.tombstones.iter().any(|t| t.id == id) {
            return Err(EntryError::DuplicateId(id));
        }
        new.validate(registry)?;
        self.entries.push(Entry {
            id,
            otp_type: new.otp_type,
            issuer: trimmed(&new.issuer),
            account: trimmed(&new.account),
            secret: new.secret,
            params: new.params,
            icon: new.icon,
            tags: clean_tags(new.tags),
            notes: new.notes,
            created_at: now_ms,
            updated_at: now_ms,
            extra: Extra::new(),
        });
        Ok(())
    }

    /// # Errors
    /// [`EntryError::NotFound`], or an invalid secret or parameters (an entry of an
    /// unsupported OTP type keeps its parameters; only its metadata can change).
    pub fn edit_entry(
        &mut self,
        id: EntryId,
        changes: EntryChanges,
        registry: &OtpRegistry,
        now_ms: i64,
    ) -> Result<(), EntryError> {
        let position = self.position_of(id)?;
        let entry = &mut self.entries[position];
        if let Some(secret) = &changes.secret {
            validate_secret(secret)?;
        }
        if let Some(params) = &changes.params {
            validate_params(registry, &entry.otp_type, params)?;
        }
        if let Some(issuer) = changes.issuer {
            entry.issuer = trimmed(&issuer);
        }
        if let Some(account) = changes.account {
            entry.account = trimmed(&account);
        }
        if let Some(secret) = changes.secret {
            entry.secret = secret;
        }
        if let Some(params) = changes.params {
            entry.params = params;
        }
        if let Some(icon) = changes.icon {
            entry.icon = icon;
        }
        if let Some(tags) = changes.tags {
            entry.tags = clean_tags(tags);
        }
        if let Some(notes) = changes.notes {
            entry.notes = notes;
        }
        entry.updated_at = now_ms;
        Ok(())
    }

    /// Applies all of `ops` or, if any is invalid, none of them.
    ///
    /// # Errors
    /// [`EntryError::DuplicateId`] if an added id is already used (by an entry, a deletion marker
    /// or another op) or an entry is replaced twice; [`EntryError::NotFound`] for a replaced entry
    /// that does not exist; or any validation error of a definition.
    pub fn apply_import(
        &mut self,
        ops: Vec<ImportOp>,
        registry: &OtpRegistry,
        now_ms: i64,
    ) -> Result<(), EntryError> {
        self.check_import(&ops, registry)?;
        for op in ops {
            match op {
                ImportOp::Add { id, new } => self.add_entry(id, new, registry, now_ms)?,
                ImportOp::Replace { id, with } => self.replace_entry(id, with, now_ms)?,
            }
        }
        Ok(())
    }

    fn check_import(&self, ops: &[ImportOp], registry: &OtpRegistry) -> Result<(), EntryError> {
        let mut claimed: Vec<EntryId> = Vec::new();
        for op in ops {
            let (id, definition, must_exist) = match op {
                ImportOp::Add { id, new } => (*id, new, false),
                ImportOp::Replace { id, with } => (*id, with, true),
            };
            definition.validate(registry)?;
            let in_use = self.position_of(id).is_ok() || self.tombstones.iter().any(|t| t.id == id);
            if must_exist && self.position_of(id).is_err() {
                return Err(EntryError::NotFound(id));
            }
            if (!must_exist && in_use) || claimed.contains(&id) {
                return Err(EntryError::DuplicateId(id));
            }
            claimed.push(id);
        }
        Ok(())
    }

    fn replace_entry(
        &mut self,
        id: EntryId,
        with: NewEntry,
        now_ms: i64,
    ) -> Result<(), EntryError> {
        let position = self.position_of(id)?;
        let entry = &mut self.entries[position];
        entry.otp_type = with.otp_type;
        entry.issuer = trimmed(&with.issuer);
        entry.account = trimmed(&with.account);
        entry.secret = with.secret;
        entry.params = with.params;
        entry.updated_at = now_ms;
        Ok(())
    }

    /// Moves a counter-based (HOTP) entry on to its next code and returns the new counter.
    ///
    /// # Errors
    /// [`EntryError::NotFound`], [`EntryError::UnknownType`], [`EntryError::NotCounterBased`],
    /// or [`EntryError::CounterOverflow`] at the largest counter value.
    pub fn advance_counter(
        &mut self,
        id: EntryId,
        registry: &OtpRegistry,
        now_ms: i64,
    ) -> Result<u64, EntryError> {
        let position = self.position_of(id)?;
        let entry = &mut self.entries[position];
        let implementation = registry
            .get(&entry.otp_type)
            .ok_or_else(|| EntryError::UnknownType(entry.otp_type.clone()))?;
        let CodeTiming::Counter(current) = implementation.timing(&entry.params)? else {
            return Err(EntryError::NotCounterBased(id));
        };
        let next = current.checked_add(1).ok_or(EntryError::CounterOverflow)?;
        entry.params = std::mem::take(&mut entry.params).with("counter", &next.to_string());
        entry.updated_at = now_ms;
        Ok(next)
    }

    /// Removes the entry (its secret is wiped as it drops) and leaves a tombstone.
    ///
    /// # Errors
    /// [`EntryError::NotFound`].
    pub fn delete_entry(&mut self, id: EntryId, now_ms: i64) -> Result<(), EntryError> {
        let position = self.position_of(id)?;
        self.entries.remove(position);
        self.tombstones.push(Tombstone {
            id,
            deleted_at: now_ms,
        });
        Ok(())
    }

    /// Moves the entry so it ends up at index `position` of the list.
    ///
    /// # Errors
    /// [`EntryError::NotFound`], or [`EntryError::PositionOutOfRange`] if `position` is not
    /// a valid index.
    pub fn move_entry(&mut self, id: EntryId, position: usize) -> Result<(), EntryError> {
        let current = self.position_of(id)?;
        let len = self.entries.len();
        if position >= len {
            return Err(EntryError::PositionOutOfRange { position, len });
        }
        let entry = self.entries.remove(current);
        self.entries.insert(position, entry);
        Ok(())
    }

    #[must_use]
    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Entries whose issuer, account or a tag contains `query`, ignoring case, in list order.
    /// A blank query matches everything.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&Entry> {
        let needle = query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|entry| needle.is_empty() || matches_text(entry, &needle))
            .collect()
    }

    /// Entries carrying exactly `tag` (ignoring case), in list order.
    #[must_use]
    pub fn with_tag(&self, tag: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            .collect()
    }

    /// Every tag in use, each once (first spelling wins), sorted ignoring case.
    #[must_use]
    pub fn all_tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> = Vec::new();
        for tag in self.entries.iter().flat_map(|entry| &entry.tags) {
            if !tags.iter().any(|known| known.eq_ignore_ascii_case(tag)) {
                tags.push(tag);
            }
        }
        tags.sort_by_key(|tag| tag.to_lowercase());
        tags
    }

    fn position_of(&self, id: EntryId) -> Result<usize, EntryError> {
        self.entries
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(EntryError::NotFound(id))
    }
}

fn matches_text(entry: &Entry, needle: &str) -> bool {
    [&entry.issuer, &entry.account]
        .into_iter()
        .chain(&entry.tags)
        .any(|text| text.to_lowercase().contains(needle))
}

fn trimmed(text: &str) -> String {
    text.trim().to_owned()
}

fn validate_secret(secret: &Secret) -> Result<(), EntryError> {
    if secret.as_bytes().is_empty() {
        return Err(EntryError::EmptySecret);
    }
    Ok(())
}

fn validate_params(
    registry: &OtpRegistry,
    otp_type: &str,
    params: &Params,
) -> Result<(), EntryError> {
    let implementation = registry
        .get(otp_type)
        .ok_or_else(|| EntryError::UnknownType(otp_type.to_owned()))?;
    Ok(implementation.validate_params(params)?)
}

/// Trims tags, drops blanks and repeats (ignoring case), keeping the first spelling and order.
fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut cleaned: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !cleaned.iter().any(|kept| kept.eq_ignore_ascii_case(tag)) {
            cleaned.push(tag.to_owned());
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::super::test_support::id;
    use super::*;

    fn registry() -> OtpRegistry {
        OtpRegistry::with_builtins()
    }

    fn new_entry(issuer: &str, account: &str) -> NewEntry {
        NewEntry {
            otp_type: "totp".into(),
            issuer: issuer.into(),
            account: account.into(),
            secret: Secret::new(vec![1; 20]),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        }
    }

    fn vault_with(accounts: &[(&str, &str)]) -> VaultData {
        let mut data = VaultData::empty(0);
        for (number, (issuer, account)) in (1_u8..).zip(accounts) {
            data.add_entry(id(number), new_entry(issuer, account), &registry(), 10)
                .unwrap();
        }
        data
    }

    fn order(data: &VaultData) -> Vec<EntryId> {
        data.entries.iter().map(|entry| entry.id).collect()
    }

    #[test]
    fn adding_stores_the_entry_with_timestamps() {
        let mut data = VaultData::empty(0);
        data.add_entry(id(1), new_entry("  GitHub ", " me "), &registry(), 42)
            .unwrap();
        let entry = data.entry(id(1)).unwrap();
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("GitHub", "me")
        );
        assert_eq!((entry.created_at, entry.updated_at), (42, 42));
        assert!(entry.extra.is_empty());
    }

    #[test]
    fn new_entries_go_to_the_end() {
        let data = vault_with(&[("A", "a"), ("B", "b"), ("C", "c")]);
        assert_eq!(order(&data), [id(1), id(2), id(3)]);
    }

    #[test]
    fn a_used_id_is_refused_even_after_deletion() {
        let mut data = vault_with(&[("A", "a")]);
        assert_eq!(
            data.add_entry(id(1), new_entry("B", "b"), &registry(), 0),
            Err(EntryError::DuplicateId(id(1)))
        );
        data.delete_entry(id(1), 5).unwrap();
        assert_eq!(
            data.add_entry(id(1), new_entry("B", "b"), &registry(), 0),
            Err(EntryError::DuplicateId(id(1)))
        );
    }

    #[test]
    fn invalid_definitions_are_refused_and_change_nothing() {
        let mut data = VaultData::empty(0);
        let mut unknown = new_entry("A", "a");
        unknown.otp_type = "vendor-x".into();
        let mut bad_params = new_entry("A", "a");
        bad_params.params = Params::new().with("digits", "99");
        let mut no_secret = new_entry("A", "a");
        no_secret.secret = Secret::new(Vec::new());

        assert_eq!(
            data.add_entry(id(1), unknown, &registry(), 0),
            Err(EntryError::UnknownType("vendor-x".into()))
        );
        assert!(matches!(
            data.add_entry(id(1), bad_params, &registry(), 0),
            Err(EntryError::InvalidParams(_))
        ));
        assert_eq!(
            data.add_entry(id(1), no_secret, &registry(), 0),
            Err(EntryError::EmptySecret)
        );
        assert_eq!(data.entries.len(), 0);
    }

    #[test]
    fn a_definition_can_be_validated_without_adding_it() {
        assert_eq!(new_entry("A", "a").validate(&registry()), Ok(()));
        let mut bad = new_entry("A", "a");
        bad.params = Params::new().with("digits", "99");
        assert!(matches!(
            bad.validate(&registry()),
            Err(EntryError::InvalidParams(_))
        ));
        let mut empty = new_entry("A", "a");
        empty.secret = Secret::new(Vec::new());
        assert_eq!(empty.validate(&registry()), Err(EntryError::EmptySecret));
    }

    #[test]
    fn tags_are_trimmed_and_deduplicated_ignoring_case() {
        let mut data = VaultData::empty(0);
        let mut new = new_entry("A", "a");
        new.tags = vec![" Work ".into(), "work".into(), " ".into(), "mail".into()];
        data.add_entry(id(1), new, &registry(), 0).unwrap();
        assert_eq!(data.entry(id(1)).unwrap().tags, ["Work", "mail"]);
    }

    #[test]
    fn editing_changes_only_the_given_fields_and_stamps_the_time() {
        let mut data = vault_with(&[("A", "a")]);
        let changes = EntryChanges {
            account: Some("renamed".into()),
            tags: Some(vec!["x".into()]),
            icon: Some(Some("icon".into())),
            ..EntryChanges::default()
        };
        data.edit_entry(id(1), changes, &registry(), 99).unwrap();
        let entry = data.entry(id(1)).unwrap();
        assert_eq!(entry.account, "renamed");
        assert_eq!(entry.issuer, "A");
        assert_eq!(entry.tags, ["x"]);
        assert_eq!(entry.icon.as_deref(), Some("icon"));
        assert_eq!((entry.created_at, entry.updated_at), (10, 99));
    }

    #[test]
    fn editing_can_remove_the_icon_and_replace_the_secret() {
        let mut data = vault_with(&[("A", "a")]);
        let first = EntryChanges {
            icon: Some(Some("icon".into())),
            ..EntryChanges::default()
        };
        data.edit_entry(id(1), first, &registry(), 11).unwrap();
        let second = EntryChanges {
            icon: Some(None),
            secret: Some(Secret::new(vec![7; 20])),
            ..EntryChanges::default()
        };
        data.edit_entry(id(1), second, &registry(), 12).unwrap();
        let entry = data.entry(id(1)).unwrap();
        assert_eq!(entry.icon, None);
        assert_eq!(entry.secret.as_bytes(), [7; 20]);
    }

    #[test]
    fn a_rejected_edit_changes_nothing() {
        let mut data = vault_with(&[("A", "a")]);
        let changes = EntryChanges {
            issuer: Some("changed".into()),
            params: Some(Params::new().with("period", "0")),
            ..EntryChanges::default()
        };
        assert!(matches!(
            data.edit_entry(id(1), changes, &registry(), 99),
            Err(EntryError::InvalidParams(_))
        ));
        let entry = data.entry(id(1)).unwrap();
        assert_eq!((entry.issuer.as_str(), entry.updated_at), ("A", 10));

        let empty_secret = EntryChanges {
            secret: Some(Secret::new(Vec::new())),
            ..EntryChanges::default()
        };
        assert_eq!(
            data.edit_entry(id(1), empty_secret, &registry(), 99),
            Err(EntryError::EmptySecret)
        );
    }

    #[test]
    fn editing_a_missing_entry_fails() {
        let mut data = VaultData::empty(0);
        assert_eq!(
            data.edit_entry(id(9), EntryChanges::default(), &registry(), 0),
            Err(EntryError::NotFound(id(9)))
        );
    }

    #[test]
    fn metadata_of_an_unsupported_type_can_change_but_its_params_cannot() {
        let mut data = crate::vault::test_support::sample_vault();
        let rename = EntryChanges {
            account: Some("robert".into()),
            ..EntryChanges::default()
        };
        data.edit_entry(id(2), rename, &registry(), 5).unwrap();
        assert_eq!(data.entry(id(2)).unwrap().account, "robert");

        let params = EntryChanges {
            params: Some(Params::new()),
            ..EntryChanges::default()
        };
        assert_eq!(
            data.edit_entry(id(2), params, &registry(), 5),
            Err(EntryError::UnknownType("vendor-x".into()))
        );
    }

    fn hotp_vault(params: Params) -> VaultData {
        let mut data = VaultData::empty(0);
        let mut new = new_entry("Bank", "me");
        new.otp_type = "hotp".into();
        new.params = params;
        data.add_entry(id(1), new, &registry(), 10).unwrap();
        data
    }

    #[test]
    fn advancing_a_hotp_entry_stores_the_next_counter() {
        let mut data = hotp_vault(Params::new().with("digits", "8"));
        assert_eq!(data.advance_counter(id(1), &registry(), 50), Ok(1));
        assert_eq!(data.advance_counter(id(1), &registry(), 60), Ok(2));
        let entry = data.entry(id(1)).unwrap();
        assert_eq!(entry.params.get("counter"), Some("2"));
        assert_eq!(entry.params.get("digits"), Some("8"));
        assert_eq!(entry.updated_at, 60);
    }

    #[test]
    fn the_largest_counter_cannot_be_advanced() {
        let mut data = hotp_vault(Params::new().with("counter", &u64::MAX.to_string()));
        assert_eq!(
            data.advance_counter(id(1), &registry(), 50),
            Err(EntryError::CounterOverflow)
        );
        let entry = data.entry(id(1)).unwrap();
        assert_eq!(
            entry.params.get("counter"),
            Some(u64::MAX.to_string().as_str())
        );
        assert_eq!(entry.updated_at, 10);
    }

    #[test]
    fn only_counter_based_entries_can_be_advanced() {
        let mut data = vault_with(&[("A", "a")]);
        assert_eq!(
            data.advance_counter(id(1), &registry(), 0),
            Err(EntryError::NotCounterBased(id(1)))
        );
        assert_eq!(
            data.advance_counter(id(9), &registry(), 0),
            Err(EntryError::NotFound(id(9)))
        );
        let mut unsupported = crate::vault::test_support::sample_vault();
        assert_eq!(
            unsupported.advance_counter(id(2), &registry(), 0),
            Err(EntryError::UnknownType("vendor-x".into()))
        );
    }

    fn definition(otp_type: &str, issuer: &str, account: &str, seed: u8) -> NewEntry {
        NewEntry {
            otp_type: otp_type.into(),
            issuer: issuer.into(),
            account: account.into(),
            secret: Secret::new(vec![seed; 20]),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        }
    }

    fn names(data: &VaultData) -> Vec<(String, String)> {
        data.entries
            .iter()
            .map(|e| (e.issuer.clone(), e.account.clone()))
            .collect()
    }

    #[test]
    fn an_import_adds_and_replaces_in_one_step() {
        let mut data = vault_with(&[("A", "a"), ("B", "b")]);
        let tag = EntryChanges {
            tags: Some(vec!["keep".into()]),
            notes: Some("keep too".into()),
            ..EntryChanges::default()
        };
        data.edit_entry(id(2), tag, &registry(), 11).unwrap();
        data.apply_import(
            vec![
                ImportOp::Add {
                    id: id(7),
                    new: definition("totp", "C", "c", 3),
                },
                ImportOp::Replace {
                    id: id(2),
                    with: definition("hotp", " B2 ", " b2 ", 9),
                },
                ImportOp::Add {
                    id: id(8),
                    new: definition("totp", "D", "d", 4),
                },
            ],
            &registry(),
            500,
        )
        .unwrap();

        assert_eq!(order(&data), [id(1), id(2), id(7), id(8)]);
        let replaced = data.entry(id(2)).unwrap();
        assert_eq!(
            (replaced.issuer.as_str(), replaced.account.as_str()),
            ("B2", "b2")
        );
        assert_eq!(replaced.otp_type, "hotp");
        assert_eq!(replaced.secret.as_bytes(), [9; 20]);
        assert_eq!(replaced.tags, ["keep"]);
        assert_eq!(replaced.notes, "keep too");
        assert_eq!((replaced.created_at, replaced.updated_at), (10, 500));
        assert_eq!(data.entry(id(7)).unwrap().created_at, 500);
    }

    #[test]
    fn one_invalid_op_means_nothing_is_imported() {
        let mut data = vault_with(&[("A", "a")]);
        let before = names(&data);
        let mut broken = definition("totp", "X", "x", 1);
        broken.params = Params::new().with("digits", "99");
        let result = data.apply_import(
            vec![
                ImportOp::Add {
                    id: id(7),
                    new: definition("totp", "C", "c", 3),
                },
                ImportOp::Replace {
                    id: id(1),
                    with: definition("totp", "A2", "a2", 5),
                },
                ImportOp::Add {
                    id: id(8),
                    new: broken,
                },
            ],
            &registry(),
            500,
        );
        assert!(matches!(result, Err(EntryError::InvalidParams(_))));
        assert_eq!(names(&data), before);
        assert_eq!(data.entry(id(1)).unwrap().secret.as_bytes(), [1; 20]);
    }

    #[test]
    fn replacing_a_missing_entry_or_reusing_an_id_is_refused_without_changes() {
        let mut data = vault_with(&[("A", "a")]);
        data.delete_entry(id(1), 20).unwrap();
        data.add_entry(id(2), new_entry("B", "b"), &registry(), 30)
            .unwrap();
        let before = names(&data);
        let add = |n: u8| ImportOp::Add {
            id: id(n),
            new: definition("totp", "N", "n", n),
        };

        assert_eq!(
            data.apply_import(
                vec![
                    add(5),
                    ImportOp::Replace {
                        id: id(9),
                        with: definition("totp", "R", "r", 1)
                    }
                ],
                &registry(),
                1,
            ),
            Err(EntryError::NotFound(id(9)))
        );
        assert_eq!(
            data.apply_import(vec![add(2)], &registry(), 1),
            Err(EntryError::DuplicateId(id(2)))
        );
        assert_eq!(
            data.apply_import(vec![add(1)], &registry(), 1),
            Err(EntryError::DuplicateId(id(1))),
            "a deleted entry's id stays reserved"
        );
        assert_eq!(
            data.apply_import(vec![add(6), add(6)], &registry(), 1),
            Err(EntryError::DuplicateId(id(6)))
        );
        let twice = vec![
            ImportOp::Replace {
                id: id(2),
                with: definition("totp", "R", "r", 1),
            },
            ImportOp::Replace {
                id: id(2),
                with: definition("totp", "S", "s", 2),
            },
        ];
        assert_eq!(
            data.apply_import(twice, &registry(), 1),
            Err(EntryError::DuplicateId(id(2)))
        );
        assert_eq!(names(&data), before);
    }

    #[test]
    fn an_empty_import_changes_nothing() {
        let mut data = vault_with(&[("A", "a")]);
        data.apply_import(Vec::new(), &registry(), 5).unwrap();
        assert_eq!(data.entry(id(1)).unwrap().updated_at, 10);
    }

    #[test]
    fn deleting_removes_the_entry_and_leaves_a_tombstone() {
        let mut data = vault_with(&[("A", "a"), ("B", "b")]);
        data.delete_entry(id(1), 77).unwrap();
        assert_eq!(order(&data), [id(2)]);
        assert_eq!(
            data.tombstones,
            [Tombstone {
                id: id(1),
                deleted_at: 77
            }]
        );
        assert_eq!(
            data.delete_entry(id(1), 78),
            Err(EntryError::NotFound(id(1)))
        );
        assert_eq!(data.tombstones.len(), 1);
    }

    #[test]
    fn moving_places_the_entry_at_the_given_index() {
        let mut data = vault_with(&[("A", "a"), ("B", "b"), ("C", "c")]);
        data.move_entry(id(3), 0).unwrap();
        assert_eq!(order(&data), [id(3), id(1), id(2)]);
        data.move_entry(id(3), 2).unwrap();
        assert_eq!(order(&data), [id(1), id(2), id(3)]);
        data.move_entry(id(2), 1).unwrap();
        assert_eq!(order(&data), [id(1), id(2), id(3)]);
    }

    #[test]
    fn moving_rejects_unknown_entries_and_bad_positions() {
        let mut data = vault_with(&[("A", "a"), ("B", "b")]);
        assert_eq!(data.move_entry(id(9), 0), Err(EntryError::NotFound(id(9))));
        assert_eq!(
            data.move_entry(id(1), 2),
            Err(EntryError::PositionOutOfRange {
                position: 2,
                len: 2
            })
        );
        assert_eq!(order(&data), [id(1), id(2)]);
    }

    #[test]
    fn search_matches_issuer_account_and_tags_ignoring_case() {
        let mut data = vault_with(&[("GitHub", "alice"), ("Google", "bob"), ("Bank", "carol")]);
        let tag = EntryChanges {
            tags: Some(vec!["Finance".into()]),
            ..EntryChanges::default()
        };
        data.edit_entry(id(3), tag, &registry(), 0).unwrap();

        let found = |query: &str| -> Vec<EntryId> {
            data.search(query).into_iter().map(|e| e.id).collect()
        };
        assert_eq!(found("GIT"), [id(1)]);
        assert_eq!(found("o"), [id(2), id(3)]);
        assert_eq!(found("bob"), [id(2)]);
        assert_eq!(found("finan"), [id(3)]);
        assert_eq!(found("nothing"), Vec::<EntryId>::new());
        assert_eq!(found("   "), [id(1), id(2), id(3)]);
    }

    #[test]
    fn search_does_not_look_into_notes_or_secrets() {
        let mut data = vault_with(&[("A", "a")]);
        let changes = EntryChanges {
            notes: Some("recovery phrase".into()),
            ..EntryChanges::default()
        };
        data.edit_entry(id(1), changes, &registry(), 0).unwrap();
        assert_eq!(data.search("recovery").len(), 0);
    }

    #[test]
    fn tags_can_be_listed_and_filtered() {
        let mut data = vault_with(&[("A", "a"), ("B", "b"), ("C", "c")]);
        for (number, tags) in [(1_u8, vec!["work", "Mail"]), (2, vec!["MAIL"]), (3, vec![])] {
            let changes = EntryChanges {
                tags: Some(tags.into_iter().map(String::from).collect()),
                ..EntryChanges::default()
            };
            data.edit_entry(id(number), changes, &registry(), 0)
                .unwrap();
        }
        assert_eq!(data.all_tags(), ["Mail", "work"]);
        let tagged: Vec<_> = data.with_tag("mail").into_iter().map(|e| e.id).collect();
        assert_eq!(tagged, [id(1), id(2)]);
        assert_eq!(data.with_tag("missing").len(), 0);
    }
}
