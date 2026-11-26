//! Joining two copies of a vault that were changed apart (ADR-0008, ADR-0018).
//!
//! For every id the newest version or deletion marker wins. The outcome does not depend on
//! which copy is called local, except for the order of the list, which stays per device.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use sha2::{Digest, Sha256};

use super::model::{Entry, EntryId, Extra, Tombstone, VaultData};

const COUNTER: &str = "counter";

/// The merged vault and which copies it differs from. Order alone is not a difference.
#[derive(Debug)]
pub struct Merged {
    pub data: VaultData,
    pub local_changed: bool,
    pub remote_changed: bool,
}

/// What one copy, or the merge so far, knows about one id.
#[derive(Debug)]
enum Record {
    Present(Box<Entry>),
    Deleted(i64),
}

#[must_use]
pub fn merge(local: VaultData, remote: VaultData) -> Merged {
    let local_before = Summary::of(&local);
    let remote_before = Summary::of(&remote);
    let local_order: Vec<EntryId> = local.entries.iter().map(|entry| entry.id).collect();

    let (created_at, updated_at) = (
        local.created_at.min(remote.created_at),
        local.updated_at.max(remote.updated_at),
    );
    let extra = merge_extra(
        (local.updated_at, local.extra),
        (remote.updated_at, remote.extra),
    );
    let mut records = BTreeMap::new();
    collect(&mut records, local.entries, local.tombstones);
    collect(&mut records, remote.entries, remote.tombstones);

    let (entries, tombstones) = split(records);
    let data = VaultData {
        created_at,
        updated_at,
        entries: in_local_order(entries, &local_order),
        tombstones,
        extra,
    };
    let after = Summary::of(&data);
    Merged {
        local_changed: after != local_before,
        remote_changed: after != remote_before,
        data,
    }
}

fn collect(
    records: &mut BTreeMap<[u8; 16], (EntryId, Record)>,
    entries: Vec<Entry>,
    tombstones: Vec<Tombstone>,
) {
    let present = entries
        .into_iter()
        .map(|entry| (entry.id, Record::Present(Box::new(entry))));
    let deleted = tombstones
        .into_iter()
        .map(|tombstone| (tombstone.id, Record::Deleted(tombstone.deleted_at)));
    for (id, record) in present.chain(deleted) {
        let key = *id.as_bytes();
        let winner = match records.remove(&key) {
            Some((_, known)) => resolve(known, record),
            None => record,
        };
        records.insert(key, (id, winner));
    }
}

fn resolve(first: Record, second: Record) -> Record {
    match (first, second) {
        (Record::Deleted(a), Record::Deleted(b)) => Record::Deleted(a.max(b)),
        (Record::Deleted(deleted_at), Record::Present(entry))
        | (Record::Present(entry), Record::Deleted(deleted_at)) => {
            if deleted_at >= entry.updated_at {
                Record::Deleted(deleted_at)
            } else {
                Record::Present(entry)
            }
        }
        (Record::Present(a), Record::Present(b)) => Record::Present(resolve_entries(a, b)),
    }
}

fn resolve_entries(first: Box<Entry>, second: Box<Entry>) -> Box<Entry> {
    let highest_counter = shared_highest_counter(&first, &second);
    let first_wins = match first.updated_at.cmp(&second.updated_at) {
        Ordering::Greater => true,
        Ordering::Less => false,
        // Arbitrary, but the same on every device.
        Ordering::Equal => digest(&first) >= digest(&second),
    };
    let mut winner = if first_wins { first } else { second };
    if let Some(counter) = highest_counter {
        winner.params = std::mem::take(&mut winner.params).with(COUNTER, &counter.to_string());
    }
    winner
}

/// A counter that has been used must not be used again, whichever version wins.
fn shared_highest_counter(first: &Entry, second: &Entry) -> Option<u64> {
    if first.otp_type != second.otp_type || first.secret.as_bytes() != second.secret.as_bytes() {
        return None;
    }
    let counter = |entry: &Entry| entry.params.get(COUNTER)?.parse::<u64>().ok();
    Some(counter(first)?.max(counter(second)?))
}

fn split(records: BTreeMap<[u8; 16], (EntryId, Record)>) -> (Vec<Entry>, Vec<Tombstone>) {
    let mut entries = Vec::new();
    let mut tombstones = Vec::new();
    for (id, record) in records.into_values() {
        match record {
            Record::Present(entry) => entries.push(*entry),
            Record::Deleted(deleted_at) => tombstones.push(Tombstone { id, deleted_at }),
        }
    }
    (entries, tombstones)
}

/// Known entries keep their places; the others follow, oldest first.
fn in_local_order(mut entries: Vec<Entry>, local_order: &[EntryId]) -> Vec<Entry> {
    let places: HashMap<EntryId, usize> = local_order
        .iter()
        .enumerate()
        .map(|(place, id)| (*id, place))
        .collect();
    entries.sort_by_key(|entry| {
        (
            places.get(&entry.id).copied().unwrap_or(usize::MAX),
            entry.created_at,
            *entry.id.as_bytes(),
        )
    });
    entries
}

fn merge_extra((first_saved, first): (i64, Extra), (second_saved, second): (i64, Extra)) -> Extra {
    let first_is_newer = match first_saved.cmp(&second_saved) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => format!("{first:?}") >= format!("{second:?}"),
    };
    let (mut older, newer) = if first_is_newer {
        (second, first)
    } else {
        (first, second)
    };
    older.extend(newer);
    older
}

/// What a copy holds, ignoring order. Entries are compared by a digest of all their fields,
/// which sees every difference without keeping a second copy of any secret.
#[derive(PartialEq, Eq)]
struct Summary {
    entries: HashSet<(EntryId, [u8; 32])>,
    tombstones: HashSet<(EntryId, i64)>,
    extra: String,
}

impl Summary {
    fn of(data: &VaultData) -> Self {
        Self {
            entries: data
                .entries
                .iter()
                .map(|entry| (entry.id, digest(entry)))
                .collect(),
            tombstones: data
                .tombstones
                .iter()
                .map(|tombstone| (tombstone.id, tombstone.deleted_at))
                .collect(),
            extra: format!("{:?}", data.extra),
        }
    }
}

fn digest(entry: &Entry) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    field(entry.secret.as_bytes());
    field(entry.otp_type.as_bytes());
    field(entry.issuer.as_bytes());
    field(entry.account.as_bytes());
    for (name, value) in entry.params.iter() {
        field(name.as_bytes());
        field(value.as_bytes());
    }
    for tag in &entry.tags {
        field(b"tag");
        field(tag.as_bytes());
    }
    field(entry.notes.as_bytes());
    if let Some(icon) = &entry.icon {
        field(b"icon");
        field(icon.as_bytes());
    }
    field(&entry.created_at.to_le_bytes());
    field(&entry.updated_at.to_le_bytes());
    field(format!("{:?}", entry.extra).as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otp::Params;
    use crate::secret::Secret;

    fn id(last: u8) -> EntryId {
        let mut bytes = [0_u8; 16];
        bytes[15] = last;
        EntryId::from_bytes(bytes)
    }

    fn entry(number: u8, account: &str, updated_at: i64) -> Entry {
        Entry {
            id: id(number),
            otp_type: "totp".into(),
            issuer: String::new(),
            account: account.into(),
            secret: Secret::new(vec![number; 20]),
            params: Params::new(),
            icon: None,
            tags: Vec::new(),
            notes: String::new(),
            created_at: i64::from(number),
            updated_at,
            extra: Extra::new(),
        }
    }

    fn counting(number: u8, counter: u64, updated_at: i64) -> Entry {
        let mut counting = entry(number, "hotp", updated_at);
        counting.otp_type = "hotp".into();
        counting.params = Params::new().with(COUNTER, &counter.to_string());
        counting
    }

    fn vault(entries: Vec<Entry>, deleted: &[(u8, i64)]) -> VaultData {
        let mut data = VaultData::empty(0);
        data.entries = entries;
        data.tombstones = deleted
            .iter()
            .map(|&(number, deleted_at)| Tombstone {
                id: id(number),
                deleted_at,
            })
            .collect();
        data
    }

    fn accounts(data: &VaultData) -> Vec<&str> {
        data.entries.iter().map(|e| e.account.as_str()).collect()
    }

    fn flags(merged: &Merged) -> (bool, bool) {
        (merged.local_changed, merged.remote_changed)
    }

    #[test]
    fn equal_copies_change_nothing() {
        let copy = || vault(vec![entry(1, "a", 10)], &[(2, 5)]);
        let merged = merge(copy(), copy());
        assert_eq!(flags(&merged), (false, false));
        assert_eq!(merged.data, copy());
    }

    #[test]
    fn each_side_receives_what_the_other_added() {
        let merged = merge(
            vault(vec![entry(1, "mine", 10)], &[]),
            vault(vec![entry(2, "theirs", 10)], &[]),
        );
        assert_eq!(accounts(&merged.data), ["mine", "theirs"]);
        assert_eq!(flags(&merged), (true, true));
    }

    #[test]
    fn the_newer_edit_wins_from_either_side() {
        let old = || vault(vec![entry(1, "old", 10)], &[]);
        let new = || vault(vec![entry(1, "new", 20)], &[]);
        let took = merge(old(), new());
        assert_eq!(accounts(&took.data), ["new"]);
        assert_eq!(flags(&took), (true, false));
        let kept = merge(new(), old());
        assert_eq!(accounts(&kept.data), ["new"]);
        assert_eq!(flags(&kept), (false, true));
    }

    #[test]
    fn edits_in_the_same_millisecond_resolve_the_same_way_on_both_devices() {
        let a = || vault(vec![entry(1, "apple", 10)], &[]);
        let b = || vault(vec![entry(1, "banana", 10)], &[]);
        let (ab, ba) = (merge(a(), b()), merge(b(), a()));
        assert_eq!(accounts(&ab.data), accounts(&ba.data));
        assert_ne!(flags(&ab), (false, false));
        assert_eq!(flags(&ab), (ba.remote_changed, ba.local_changed));
    }

    #[test]
    fn a_deletion_removes_the_older_version_and_wins_a_tie() {
        for edited_at in [10, 20] {
            let merged = merge(
                vault(vec![entry(1, "a", edited_at)], &[]),
                vault(vec![], &[(1, 20)]),
            );
            assert_eq!(accounts(&merged.data), [""; 0]);
            assert_eq!(merged.data.tombstones.len(), 1);
            assert_eq!(flags(&merged), (true, false));
        }
    }

    #[test]
    fn an_edit_after_a_deletion_brings_the_account_back() {
        let merged = merge(
            vault(vec![], &[(1, 20)]),
            vault(vec![entry(1, "again", 30)], &[]),
        );
        assert_eq!(accounts(&merged.data), ["again"]);
        assert_eq!(merged.data.tombstones, []);
    }

    #[test]
    fn the_later_of_two_deletion_times_is_kept() {
        let merged = merge(vault(vec![], &[(1, 20)]), vault(vec![], &[(1, 40)]));
        assert_eq!(merged.data.tombstones[0].deleted_at, 40);
        assert_eq!(flags(&merged), (true, false));
    }

    #[test]
    fn a_counter_never_goes_back() {
        let merged = merge(
            vault(vec![counting(1, 9, 10)], &[]),
            vault(vec![counting(1, 4, 20)], &[]),
        );
        let kept = &merged.data.entries[0];
        assert_eq!((kept.params.get(COUNTER), kept.updated_at), (Some("9"), 20));
        assert_eq!(flags(&merged), (true, true));
    }

    #[test]
    fn a_replaced_secret_starts_its_own_counter() {
        let mut replaced = counting(1, 0, 20);
        replaced.secret = Secret::new(vec![99; 20]);
        let merged = merge(
            vault(vec![counting(1, 9, 10)], &[]),
            vault(vec![replaced], &[]),
        );
        assert_eq!(merged.data.entries[0].params.get(COUNTER), Some("0"));
    }

    #[test]
    fn a_different_order_alone_is_not_a_change_and_each_side_keeps_its_own() {
        let merged = merge(
            vault(vec![entry(2, "b", 1), entry(1, "a", 1)], &[]),
            vault(vec![entry(1, "a", 1), entry(2, "b", 1)], &[]),
        );
        assert_eq!(accounts(&merged.data), ["b", "a"]);
        assert_eq!(flags(&merged), (false, false));
    }

    #[test]
    fn accounts_new_to_this_device_follow_oldest_first() {
        let merged = merge(
            vault(vec![entry(9, "local", 1)], &[]),
            vault(vec![entry(3, "younger", 1), entry(2, "older", 1)], &[]),
        );
        assert_eq!(accounts(&merged.data), ["local", "older", "younger"]);
    }

    #[test]
    fn unknown_fields_from_both_sides_survive_and_the_later_save_wins() {
        let with = |saved: i64, pairs: &[(&str, &str)]| {
            let mut data = vault(vec![], &[]);
            data.updated_at = saved;
            for (key, value) in pairs {
                data.extra
                    .insert((*key).into(), ciborium::Value::Text((*value).into()));
            }
            data
        };
        let merged = merge(
            with(10, &[("shared", "early"), ("mine", "1")]),
            with(20, &[("shared", "late"), ("theirs", "2")]),
        );
        let text = |key: &str| merged.data.extra[key].as_text().unwrap();
        assert_eq!(
            (text("shared"), text("mine"), text("theirs")),
            ("late", "1", "2")
        );
        assert_eq!(merged.data.updated_at, 20);
    }

    #[test]
    fn merging_again_after_a_merge_settles() {
        let a = || {
            vault(
                vec![entry(1, "a", 30), counting(2, 7, 10), entry(4, "d", 5)],
                &[(3, 8)],
            )
        };
        let b = || {
            vault(
                vec![entry(1, "b", 20), counting(2, 3, 40), entry(3, "c", 4)],
                &[(4, 50)],
            )
        };
        let first = merge(a(), b()).data;
        let again = merge(merge(a(), b()).data, merge(b(), a()).data);
        assert_eq!(flags(&again), (false, false));
        assert_eq!(accounts(&again.data), accounts(&first));
    }
}
