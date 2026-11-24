//! Deciding what an import would do, before it does anything (ADR-0004).
//!
//! Two accounts have the same *name* when issuer and account match after trimming and ignoring
//! case. Same name and same secret is a duplicate; same name with a different secret is a
//! conflict. Planning only reads the vault; [`build_ops`] turns the user's decisions into the
//! operations the core applies in one atomic step.

use std::collections::HashMap;

use mfa_core::vault::{EntryId, ImportOp, NewEntry, VaultData};
use thiserror::Error;

/// How one incoming account relates to what the vault already holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeStatus {
    New,
    /// Same name and secret as an entry in the vault.
    DuplicateOfVault(EntryId),
    /// Same name and secret as an earlier item of the same import (its index).
    DuplicateOfEarlierItem(usize),
    /// Same name as an entry in the vault, but a different secret.
    ConflictWithVault(EntryId),
    /// The same secret as a vault entry that has a different name: most likely the same account
    /// entered another way (an export without issuers, say).
    SameSecretUnderOtherName(EntryId),
}

/// What to do with one incoming account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Skip,
    /// Add it as a new entry (for duplicates and conflicts this keeps both).
    Add,
    /// Overwrite the vault entry it matched.
    Replace,
}

impl Decision {
    /// New accounts and conflicts are added (nothing is lost either way); duplicates, and the
    /// same secret under another name, are skipped.
    #[must_use]
    pub const fn default_for(status: MergeStatus) -> Self {
        match status {
            MergeStatus::New | MergeStatus::ConflictWithVault(_) => Self::Add,
            MergeStatus::DuplicateOfVault(_)
            | MergeStatus::DuplicateOfEarlierItem(_)
            | MergeStatus::SameSecretUnderOtherName(_) => Self::Skip,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MergeError {
    #[error("item {0} cannot replace anything: it matches no entry in the vault")]
    NothingToReplace(usize),
    #[error("items {0} and {1} would both replace the same entry")]
    ReplacesTheSameEntryTwice(usize, usize),
}

type Name = (String, String);

fn name_of(entry: &NewEntry) -> Name {
    (
        entry.issuer.trim().to_lowercase(),
        entry.account.trim().to_lowercase(),
    )
}

/// One status per item of `incoming`, in order.
#[must_use]
pub fn plan_merge(vault: &VaultData, incoming: &[NewEntry]) -> Vec<MergeStatus> {
    let mut in_vault: HashMap<Name, Vec<usize>> = HashMap::new();
    for (index, entry) in vault.entries.iter().enumerate() {
        let name = (
            entry.issuer.trim().to_lowercase(),
            entry.account.trim().to_lowercase(),
        );
        in_vault.entry(name).or_default().push(index);
    }
    let mut earlier: HashMap<Name, Vec<usize>> = HashMap::new();
    let mut statuses = Vec::with_capacity(incoming.len());
    for (index, item) in incoming.iter().enumerate() {
        let name = name_of(item);
        statuses.push(classify(
            vault,
            incoming,
            item,
            in_vault.get(&name),
            earlier.get(&name),
        ));
        earlier.entry(name).or_default().push(index);
    }
    statuses
}

fn classify(
    vault: &VaultData,
    incoming: &[NewEntry],
    item: &NewEntry,
    vault_matches: Option<&Vec<usize>>,
    earlier_matches: Option<&Vec<usize>>,
) -> MergeStatus {
    let same_in_vault = vault_matches
        .into_iter()
        .flatten()
        .map(|index| &vault.entries[*index])
        .find(|entry| entry.secret == item.secret);
    if let Some(entry) = same_in_vault {
        return MergeStatus::DuplicateOfVault(entry.id);
    }
    let same_earlier = earlier_matches
        .into_iter()
        .flatten()
        .find(|index| incoming[**index].secret == item.secret);
    if let Some(index) = same_earlier {
        return MergeStatus::DuplicateOfEarlierItem(*index);
    }
    if let Some(index) = vault_matches.and_then(|matches| matches.first()) {
        return MergeStatus::ConflictWithVault(vault.entries[*index].id);
    }
    vault
        .entries
        .iter()
        .find(|entry| entry.secret == item.secret)
        .map_or(MergeStatus::New, |entry| {
            MergeStatus::SameSecretUnderOtherName(entry.id)
        })
}

/// An incoming account with its status and the user's decision.
pub struct PlannedItem {
    pub definition: NewEntry,
    pub status: MergeStatus,
    pub decision: Decision,
}

/// The operations for every item that is not skipped, in item order. `new_id` supplies the
/// id of each added entry.
///
/// # Errors
/// [`MergeError`] if an item is set to replace but matched nothing, or two items would
/// replace the same entry.
pub fn build_ops(
    items: Vec<PlannedItem>,
    mut new_id: impl FnMut() -> EntryId,
) -> Result<Vec<ImportOp>, MergeError> {
    let mut ops = Vec::new();
    let mut replaced: Vec<(EntryId, usize)> = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        match item.decision {
            Decision::Skip => {}
            Decision::Add => ops.push(ImportOp::Add {
                id: new_id(),
                new: item.definition,
            }),
            Decision::Replace => {
                let id = match item.status {
                    MergeStatus::DuplicateOfVault(id)
                    | MergeStatus::ConflictWithVault(id)
                    | MergeStatus::SameSecretUnderOtherName(id) => id,
                    MergeStatus::New | MergeStatus::DuplicateOfEarlierItem(_) => {
                        return Err(MergeError::NothingToReplace(index));
                    }
                };
                if let Some((_, first)) = replaced.iter().find(|(known, _)| *known == id) {
                    return Err(MergeError::ReplacesTheSameEntryTwice(*first, index));
                }
                replaced.push((id, index));
                ops.push(ImportOp::Replace {
                    id,
                    with: item.definition,
                });
            }
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mfa_core::otp::{OtpRegistry, Params};
    use mfa_core::secret::Secret;

    fn id(n: u8) -> EntryId {
        EntryId::from_bytes([n; 16])
    }

    fn def(issuer: &str, account: &str, seed: u8) -> NewEntry {
        NewEntry {
            otp_type: "totp".into(),
            issuer: issuer.into(),
            account: account.into(),
            secret: Secret::new(vec![seed; 20]),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
        }
    }

    fn vault(entries: &[(&str, &str, u8)]) -> VaultData {
        let registry = OtpRegistry::with_builtins();
        let mut data = VaultData::empty(0);
        for (n, (issuer, account, seed)) in (1_u8..).zip(entries) {
            data.add_entry(id(n), def(issuer, account, *seed), &registry, 0)
                .unwrap();
        }
        data
    }

    #[test]
    fn everything_is_new_in_an_empty_vault() {
        let plan = plan_merge(&VaultData::empty(0), &[def("A", "a", 1), def("B", "b", 2)]);
        assert_eq!(plan, [MergeStatus::New, MergeStatus::New]);
    }

    #[test]
    fn same_name_and_secret_is_a_duplicate_of_the_vault_entry() {
        let plan = plan_merge(&vault(&[("A", "a", 1), ("B", "b", 2)]), &[def("B", "b", 2)]);
        assert_eq!(plan, [MergeStatus::DuplicateOfVault(id(2))]);
    }

    #[test]
    fn same_name_with_another_secret_is_a_conflict() {
        let plan = plan_merge(&vault(&[("A", "a", 1)]), &[def("A", "a", 9)]);
        assert_eq!(plan, [MergeStatus::ConflictWithVault(id(1))]);
    }

    #[test]
    fn names_compare_ignoring_case_and_surrounding_spaces() {
        let plan = plan_merge(
            &vault(&[("GitHub", "Alice", 1)]),
            &[def(" github ", "ALICE ", 1)],
        );
        assert_eq!(plan, [MergeStatus::DuplicateOfVault(id(1))]);
    }

    #[test]
    fn the_same_secret_under_a_different_name_is_flagged() {
        let plan = plan_merge(
            &vault(&[("A", "a", 1)]),
            &[def("A", "other", 1), def("Z", "a", 1)],
        );
        assert_eq!(
            plan,
            [
                MergeStatus::SameSecretUnderOtherName(id(1)),
                MergeStatus::SameSecretUnderOtherName(id(1))
            ]
        );
    }

    #[test]
    fn a_name_conflict_outranks_a_same_secret_hint() {
        let plan = plan_merge(&vault(&[("A", "a", 1), ("B", "b", 2)]), &[def("A", "a", 2)]);
        assert_eq!(plan, [MergeStatus::ConflictWithVault(id(1))]);
    }

    #[test]
    fn a_repeat_inside_the_file_is_a_duplicate_of_the_earlier_item() {
        let plan = plan_merge(
            &VaultData::empty(0),
            &[def("A", "a", 1), def("B", "b", 2), def("A", "a", 1)],
        );
        assert_eq!(
            plan,
            [
                MergeStatus::New,
                MergeStatus::New,
                MergeStatus::DuplicateOfEarlierItem(0)
            ]
        );
    }

    #[test]
    fn two_secrets_under_one_name_in_the_file_are_both_new() {
        let plan = plan_merge(&VaultData::empty(0), &[def("A", "a", 1), def("A", "a", 2)]);
        assert_eq!(plan, [MergeStatus::New, MergeStatus::New]);
    }

    #[test]
    fn a_vault_duplicate_wins_over_a_conflict_with_another_vault_entry_of_that_name() {
        let plan = plan_merge(&vault(&[("A", "a", 1), ("A", "a", 2)]), &[def("A", "a", 2)]);
        assert_eq!(plan, [MergeStatus::DuplicateOfVault(id(2))]);
    }

    #[test]
    fn planning_does_not_touch_the_vault() {
        let data = vault(&[("A", "a", 1)]);
        let before = (data.entries.len(), data.updated_at);
        let _ = plan_merge(&data, &[def("A", "a", 2), def("B", "b", 3)]);
        assert_eq!((data.entries.len(), data.updated_at), before);
    }

    #[test]
    fn large_imports_plan_quickly() {
        let incoming: Vec<NewEntry> = (0..5000_u32)
            .map(|n| {
                def(
                    "Issuer",
                    &format!("account{n}"),
                    u8::try_from(n % 250).unwrap(),
                )
            })
            .collect();
        let plan = plan_merge(&VaultData::empty(0), &incoming);
        assert_eq!(plan.len(), 5000);
        assert!(plan.iter().all(|status| *status == MergeStatus::New));
    }

    #[test]
    fn defaults_add_new_and_conflicting_accounts_and_skip_duplicates() {
        assert_eq!(Decision::default_for(MergeStatus::New), Decision::Add);
        assert_eq!(
            Decision::default_for(MergeStatus::ConflictWithVault(id(1))),
            Decision::Add
        );
        assert_eq!(
            Decision::default_for(MergeStatus::DuplicateOfVault(id(1))),
            Decision::Skip
        );
        assert_eq!(
            Decision::default_for(MergeStatus::DuplicateOfEarlierItem(0)),
            Decision::Skip
        );
        assert_eq!(
            Decision::default_for(MergeStatus::SameSecretUnderOtherName(id(1))),
            Decision::Skip
        );
    }

    fn item(status: MergeStatus, decision: Decision, seed: u8) -> PlannedItem {
        PlannedItem {
            definition: def("I", "i", seed),
            status,
            decision,
        }
    }

    fn counter() -> impl FnMut() -> EntryId {
        let mut next = 100;
        move || {
            next += 1;
            id(next)
        }
    }

    #[test]
    fn decisions_become_operations_in_item_order() {
        let ops = build_ops(
            vec![
                item(MergeStatus::New, Decision::Add, 1),
                item(MergeStatus::DuplicateOfVault(id(5)), Decision::Skip, 2),
                item(MergeStatus::ConflictWithVault(id(6)), Decision::Replace, 3),
                item(MergeStatus::ConflictWithVault(id(7)), Decision::Add, 4),
                item(MergeStatus::DuplicateOfVault(id(8)), Decision::Replace, 5),
            ],
            counter(),
        )
        .unwrap();
        let shape: Vec<_> = ops
            .iter()
            .map(|op| match op {
                ImportOp::Add { id, .. } => ("add", *id),
                ImportOp::Replace { id, .. } => ("replace", *id),
            })
            .collect();
        assert_eq!(
            shape,
            [
                ("add", id(101)),
                ("replace", id(6)),
                ("add", id(102)),
                ("replace", id(8))
            ]
        );
    }

    #[test]
    fn skipping_everything_gives_no_operations() {
        let ops = build_ops(vec![item(MergeStatus::New, Decision::Skip, 1)], counter()).unwrap();
        assert_eq!(ops.len(), 0);
    }

    #[test]
    fn replacing_needs_a_vault_entry_to_replace() {
        assert_eq!(
            build_ops(
                vec![
                    item(MergeStatus::New, Decision::Add, 1),
                    item(MergeStatus::New, Decision::Replace, 2)
                ],
                counter()
            ),
            Err(MergeError::NothingToReplace(1))
        );
        assert_eq!(
            build_ops(
                vec![item(
                    MergeStatus::DuplicateOfEarlierItem(0),
                    Decision::Replace,
                    2
                )],
                counter()
            ),
            Err(MergeError::NothingToReplace(0))
        );
    }

    #[test]
    fn two_items_cannot_replace_the_same_entry() {
        assert_eq!(
            build_ops(
                vec![
                    item(MergeStatus::ConflictWithVault(id(3)), Decision::Replace, 1),
                    item(MergeStatus::New, Decision::Add, 2),
                    item(MergeStatus::ConflictWithVault(id(3)), Decision::Replace, 3),
                ],
                counter(),
            ),
            Err(MergeError::ReplacesTheSameEntryTwice(0, 2))
        );
    }

    #[test]
    fn a_whole_import_flows_through_plan_decide_and_apply() {
        let registry = OtpRegistry::with_builtins();
        let mut data = vault(&[("A", "a", 1), ("B", "b", 2)]);
        let incoming = vec![
            def("A", "a", 1),
            def("B", "b", 9),
            def("C", "c", 3),
            def("C", "c", 3),
        ];
        let statuses = plan_merge(&data, &incoming);
        let items = incoming
            .into_iter()
            .zip(statuses)
            .map(|(definition, status)| PlannedItem {
                decision: Decision::default_for(status),
                definition,
                status,
            })
            .collect();
        let ops = build_ops(items, counter()).unwrap();
        data.apply_import(ops, &registry, 77).unwrap();
        let accounts: Vec<_> = data.entries.iter().map(|e| e.account.as_str()).collect();
        assert_eq!(
            accounts,
            ["a", "b", "b", "c"],
            "duplicates skipped, the conflict kept both, the new one added once"
        );
    }
}
