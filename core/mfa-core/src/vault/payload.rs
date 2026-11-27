//! CBOR payload and padding (ADR-0011, docs/specs/vault-format-v1.md).

use std::collections::BTreeMap;

use ciborium::value::Integer;
use ciborium::Value;
use zeroize::{Zeroize, Zeroizing};

use super::error::VaultError;
use super::model::{Entry, EntryId, Extra, Tombstone, VaultData};
use super::sync_secret::SyncSecret;
use super::sync_state::SyncState;
use crate::otp::Params;
use crate::secret::Secret;

const PAYLOAD_VERSION: i64 = 1;
const SHARED_PAYLOAD_VERSION: i64 = 1;
const PAD_BLOCK: usize = 1024;
const PAD_MARKER: u8 = 0x80;

const PAYLOAD_KEYS: [&str; 6] = [
    "v",
    "created_at",
    "updated_at",
    "entries",
    "tombstones",
    SYNC_KEY,
];
const SYNC_KEY: &str = "sync";
const ENTRY_KEYS: [&str; 11] = [
    "id",
    "otp_type",
    "issuer",
    "account",
    "secret",
    "params",
    "icon",
    "tags",
    "notes",
    "created_at",
    "updated_at",
];

fn invalid(message: impl Into<String>) -> VaultError {
    VaultError::InvalidPayload(message.into())
}

/// Serialises to deterministic CBOR (map keys in ascending order).
pub(super) fn encode(data: &VaultData) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    reject_colliding_extras(data)?;
    let mut fields = shared_fields(data);
    fields.insert("v".to_owned(), int(PAYLOAD_VERSION));
    if let Some(sync) = &data.sync {
        fields.insert(SYNC_KEY.to_owned(), sync_value(sync));
    }
    let tree = WipedOnDrop(map_value(fields, &data.extra));
    to_cbor(&tree.0)
}

pub(super) fn decode(bytes: &[u8]) -> Result<VaultData, VaultError> {
    let mut fields = Fields::parse(from_cbor(bytes)?, "payload")?;
    let version = fields.int("v")?;
    if version != PAYLOAD_VERSION {
        return Err(VaultError::UnsupportedPayloadVersion(version));
    }
    let sync = fields.0.remove(SYNC_KEY).map(decode_sync).transpose()?;
    Ok(VaultData {
        sync,
        ..decode_vault_fields(fields)?
    })
}

/// The fields every copy of a vault shares; `sync` is left empty.
fn decode_vault_fields(mut fields: Fields) -> Result<VaultData, VaultError> {
    let created_at = fields.int("created_at")?;
    let updated_at = fields.int("updated_at")?;
    let entries = fields
        .array("entries")?
        .into_iter()
        .map(decode_entry)
        .collect::<Result<_, _>>()?;
    let tombstones = fields
        .array("tombstones")?
        .into_iter()
        .map(decode_tombstone)
        .collect::<Result<_, _>>()?;
    Ok(VaultData {
        created_at,
        updated_at,
        entries,
        tombstones,
        sync: None,
        extra: fields.into_extra(),
    })
}

/// The payload of a sync file: the vault's shared data under a revision number.
pub(super) fn encode_shared(
    data: &VaultData,
    revision: u64,
) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    reject_colliding_extras(data)?;
    let vault = map_value(shared_fields(data), &data.extra);
    let tree = WipedOnDrop(map_value(
        BTreeMap::from([
            ("v".to_owned(), int(SHARED_PAYLOAD_VERSION)),
            ("revision".to_owned(), Value::Integer(revision.into())),
            ("vault".to_owned(), vault),
        ]),
        &Extra::new(),
    ));
    to_cbor(&tree.0)
}

pub(super) fn decode_shared(bytes: &[u8]) -> Result<(u64, VaultData), VaultError> {
    let mut fields = Fields::parse(from_cbor(bytes)?, "payload")?;
    let version = fields.int("v")?;
    if version != SHARED_PAYLOAD_VERSION {
        return Err(VaultError::UnsupportedPayloadVersion(version));
    }
    let revision = u64::try_from(fields.int("revision")?)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| invalid("`revision` must be 1 or more"))?;
    let vault = Fields::parse(fields.take("vault")?, "vault")?;
    if vault.0.contains_key(SYNC_KEY) {
        return Err(invalid(
            "a sync file must not hold a device's sync settings",
        ));
    }
    // A field this version cannot keep would be lost on the next write.
    if !fields.into_extra().is_empty() {
        return Err(invalid("unknown field beside the vault"));
    }
    Ok((revision, decode_vault_fields(vault)?))
}

fn to_cbor(value: &Value) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    let mut bytes = Zeroizing::new(Vec::new());
    ciborium::into_writer(value, &mut *bytes).map_err(|_| invalid("encoding failed"))?;
    Ok(bytes)
}

fn from_cbor(bytes: &[u8]) -> Result<Value, VaultError> {
    let mut reader = bytes;
    let value = ciborium::from_reader(&mut reader).map_err(|_| invalid("not valid CBOR"))?;
    if !reader.is_empty() {
        return Err(invalid("trailing data after the payload"));
    }
    Ok(value)
}

/// Appends `0x80` and zeros up to the next multiple of 1024 bytes.
pub(super) fn pad(payload: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut padded = Zeroizing::new(Vec::with_capacity(payload.len() + PAD_BLOCK));
    padded.extend_from_slice(payload);
    padded.push(PAD_MARKER);
    let padded_length = padded.len().next_multiple_of(PAD_BLOCK);
    padded.resize(padded_length, 0);
    padded
}

pub(super) fn unpad(padded: &[u8]) -> Result<&[u8], VaultError> {
    let marker = padded
        .iter()
        .rposition(|byte| *byte != 0)
        .filter(|index| padded[*index] == PAD_MARKER)
        .ok_or_else(|| invalid("bad padding"))?;
    Ok(&padded[..marker])
}

// ---- encoding ----

/// Wipes secret bytes in the intermediate tree, however encoding ends.
struct WipedOnDrop(Value);

impl Drop for WipedOnDrop {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

fn wipe(value: &mut Value) {
    match value {
        Value::Bytes(bytes) => bytes.zeroize(),
        Value::Array(items) => items.iter_mut().for_each(wipe),
        Value::Map(pairs) => pairs.iter_mut().for_each(|(key, val)| {
            wipe(key);
            wipe(val);
        }),
        Value::Tag(_, inner) => wipe(inner),
        _ => {}
    }
}

fn reject_colliding_extras(data: &VaultData) -> Result<(), VaultError> {
    let collides = |extra: &Extra, known: &[&str]| known.iter().any(|key| extra.contains_key(*key));
    if collides(&data.extra, &PAYLOAD_KEYS)
        || data
            .entries
            .iter()
            .any(|entry| collides(&entry.extra, &ENTRY_KEYS))
    {
        return Err(invalid("an unknown field collides with a known field"));
    }
    Ok(())
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn int(value: i64) -> Value {
    Value::Integer(Integer::from(value))
}

fn map_value(mut fields: BTreeMap<String, Value>, extra: &Extra) -> Value {
    fields.extend(
        extra
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (Value::Text(key), value))
            .collect(),
    )
}

fn shared_fields(data: &VaultData) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("created_at".to_owned(), int(data.created_at)),
        ("updated_at".to_owned(), int(data.updated_at)),
        (
            "entries".to_owned(),
            Value::Array(data.entries.iter().map(entry_value).collect()),
        ),
        (
            "tombstones".to_owned(),
            Value::Array(data.tombstones.iter().map(tombstone_value).collect()),
        ),
    ])
}

fn sync_value(sync: &SyncState) -> Value {
    let fields = BTreeMap::from([
        ("sync_id".to_owned(), Value::Bytes(sync.sync_id.to_vec())),
        (
            "master_key".to_owned(),
            Value::Bytes(sync.master_key.to_vec()),
        ),
        (
            "sync_secret".to_owned(),
            Value::Bytes(sync.sync_secret.as_bytes().to_vec()),
        ),
        ("location".to_owned(), text(&sync.location)),
        (
            "highest_revision".to_owned(),
            Value::Integer(sync.highest_revision.into()),
        ),
    ]);
    map_value(fields, &Extra::new())
}

fn entry_value(entry: &Entry) -> Value {
    let mut fields = BTreeMap::from([
        ("id".to_owned(), Value::Bytes(entry.id.as_bytes().to_vec())),
        ("otp_type".to_owned(), text(&entry.otp_type)),
        ("issuer".to_owned(), text(&entry.issuer)),
        ("account".to_owned(), text(&entry.account)),
        (
            "secret".to_owned(),
            Value::Bytes(entry.secret.as_bytes().to_vec()),
        ),
        ("params".to_owned(), params_value(&entry.params)),
        (
            "tags".to_owned(),
            Value::Array(entry.tags.iter().map(|tag| text(tag)).collect()),
        ),
        ("notes".to_owned(), text(&entry.notes)),
        ("created_at".to_owned(), int(entry.created_at)),
        ("updated_at".to_owned(), int(entry.updated_at)),
    ]);
    if let Some(icon) = &entry.icon {
        fields.insert("icon".to_owned(), text(icon));
    }
    map_value(fields, &entry.extra)
}

fn params_value(params: &Params) -> Value {
    Value::Map(
        params
            .iter()
            .map(|(name, value)| (text(name), text(value)))
            .collect(),
    )
}

fn tombstone_value(tombstone: &Tombstone) -> Value {
    map_value(
        BTreeMap::from([
            (
                "id".to_owned(),
                Value::Bytes(tombstone.id.as_bytes().to_vec()),
            ),
            ("deleted_at".to_owned(), int(tombstone.deleted_at)),
        ]),
        &Extra::new(),
    )
}

// ---- decoding ----

struct Fields(BTreeMap<String, Value>);

impl Fields {
    fn parse(value: Value, what: &str) -> Result<Self, VaultError> {
        let Value::Map(pairs) = value else {
            return Err(invalid(format!("{what} must be a map")));
        };
        let mut fields = BTreeMap::new();
        for (key, value) in pairs {
            let Value::Text(key) = key else {
                return Err(invalid(format!("{what} has a non-text key")));
            };
            if fields.insert(key.clone(), value).is_some() {
                return Err(invalid(format!("{what} repeats the key `{key}`")));
            }
        }
        Ok(Self(fields))
    }

    fn take(&mut self, key: &str) -> Result<Value, VaultError> {
        self.0
            .remove(key)
            .ok_or_else(|| invalid(format!("missing field `{key}`")))
    }

    fn text(&mut self, key: &str) -> Result<String, VaultError> {
        match self.take(key)? {
            Value::Text(text) => Ok(text),
            _ => Err(wrong_type(key, "text")),
        }
    }

    fn optional_text(&mut self, key: &str) -> Result<Option<String>, VaultError> {
        match self.0.remove(key) {
            None => Ok(None),
            Some(Value::Text(text)) => Ok(Some(text)),
            Some(_) => Err(wrong_type(key, "text")),
        }
    }

    fn bytes(&mut self, key: &str) -> Result<Vec<u8>, VaultError> {
        match self.take(key)? {
            Value::Bytes(bytes) => Ok(bytes),
            _ => Err(wrong_type(key, "bytes")),
        }
    }

    fn fixed_bytes<const N: usize>(&mut self, key: &str) -> Result<[u8; N], VaultError> {
        let bytes = Zeroizing::new(self.bytes(key)?);
        <[u8; N]>::try_from(bytes.as_slice())
            .map_err(|_| invalid(format!("`{key}` must be {N} bytes")))
    }

    fn id(&mut self, key: &str) -> Result<EntryId, VaultError> {
        let bytes = self.bytes(key)?;
        <[u8; 16]>::try_from(bytes.as_slice())
            .map(EntryId::from_bytes)
            .map_err(|_| invalid(format!("`{key}` must be 16 bytes")))
    }

    fn int(&mut self, key: &str) -> Result<i64, VaultError> {
        match self.take(key)? {
            Value::Integer(integer) => {
                i64::try_from(integer).map_err(|_| invalid(format!("`{key}` is out of range")))
            }
            _ => Err(wrong_type(key, "an integer")),
        }
    }

    fn array(&mut self, key: &str) -> Result<Vec<Value>, VaultError> {
        match self.take(key)? {
            Value::Array(items) => Ok(items),
            _ => Err(wrong_type(key, "an array")),
        }
    }

    fn into_extra(self) -> Extra {
        self.0
    }
}

fn wrong_type(key: &str, expected: &str) -> VaultError {
    invalid(format!("`{key}` must be {expected}"))
}

fn decode_sync(value: Value) -> Result<SyncState, VaultError> {
    let mut fields = Fields::parse(value, "sync")?;
    let state = SyncState {
        sync_id: fields.fixed_bytes("sync_id")?,
        master_key: Zeroizing::new(fields.fixed_bytes("master_key")?),
        sync_secret: SyncSecret::from_bytes(fields.fixed_bytes("sync_secret")?),
        location: fields.text("location")?,
        highest_revision: u64::try_from(fields.int("highest_revision")?)
            .map_err(|_| invalid("`highest_revision` is out of range"))?,
    };
    if !fields.into_extra().is_empty() {
        return Err(invalid("unknown sync setting"));
    }
    Ok(state)
}

fn decode_entry(value: Value) -> Result<Entry, VaultError> {
    let mut fields = Fields::parse(value, "entry")?;
    Ok(Entry {
        id: fields.id("id")?,
        otp_type: fields.text("otp_type")?,
        issuer: fields.text("issuer")?,
        account: fields.text("account")?,
        secret: Secret::new(fields.bytes("secret")?),
        params: decode_params(fields.take("params")?)?,
        icon: fields.optional_text("icon")?,
        tags: decode_tags(fields.array("tags")?)?,
        notes: fields.text("notes")?,
        created_at: fields.int("created_at")?,
        updated_at: fields.int("updated_at")?,
        extra: fields.into_extra(),
    })
}

fn decode_tombstone(value: Value) -> Result<Tombstone, VaultError> {
    let mut fields = Fields::parse(value, "tombstone")?;
    Ok(Tombstone {
        id: fields.id("id")?,
        deleted_at: fields.int("deleted_at")?,
    })
}

fn decode_params(value: Value) -> Result<Params, VaultError> {
    let Value::Map(pairs) = value else {
        return Err(wrong_type("params", "a map"));
    };
    pairs
        .into_iter()
        .try_fold(Params::new(), |params, pair| match pair {
            (Value::Text(name), Value::Text(value)) => Ok(params.with(&name, &value)),
            _ => Err(invalid("`params` must map text to text")),
        })
}

fn decode_tags(items: Vec<Value>) -> Result<Vec<String>, VaultError> {
    items
        .into_iter()
        .map(|item| match item {
            Value::Text(tag) => Ok(tag),
            _ => Err(invalid("`tags` must contain only text")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{id, sample_vault};
    use super::*;
    use crate::vault::test_support::sync_state;

    fn encoded(value: &Value) -> Vec<u8> {
        let mut bytes = Vec::new();
        ciborium::into_writer(value, &mut bytes).unwrap();
        bytes
    }

    fn base_payload() -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("v".into(), int(1)),
            ("created_at".into(), int(1)),
            ("updated_at".into(), int(2)),
            ("entries".into(), Value::Array(Vec::new())),
            ("tombstones".into(), Value::Array(Vec::new())),
        ])
    }

    fn payload_bytes(fields: BTreeMap<String, Value>) -> Vec<u8> {
        encoded(&map_value(fields, &Extra::new()))
    }

    #[test]
    fn round_trips_entries_tombstones_and_unknown_fields() {
        let original = sample_vault();
        let decoded = decode(&encode(&original).unwrap()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn encoding_is_deterministic() {
        let data = sample_vault();
        assert_eq!(*encode(&data).unwrap(), *encode(&data).unwrap());
    }

    #[test]
    fn empty_vault_round_trips() {
        let data = VaultData::empty(42);
        assert_eq!(decode(&encode(&data).unwrap()).unwrap(), data);
    }

    #[test]
    fn unknown_field_names_that_shadow_known_ones_are_refused() {
        let mut data = sample_vault();
        data.entries[0].extra.insert("secret".into(), Value::Null);
        assert!(matches!(encode(&data), Err(VaultError::InvalidPayload(_))));
    }

    #[test]
    fn newer_payload_versions_are_refused() {
        let mut fields = base_payload();
        fields.insert("v".into(), int(2));
        assert_eq!(
            decode(&payload_bytes(fields)),
            Err(VaultError::UnsupportedPayloadVersion(2))
        );
    }

    #[test]
    fn missing_and_mistyped_fields_are_refused() {
        let mut missing = base_payload();
        missing.remove("entries");
        assert!(
            matches!(decode(&payload_bytes(missing)), Err(VaultError::InvalidPayload(m)) if m.contains("entries"))
        );

        let mut mistyped = base_payload();
        mistyped.insert("created_at".into(), text("yesterday"));
        assert!(matches!(
            decode(&payload_bytes(mistyped)),
            Err(VaultError::InvalidPayload(_))
        ));
    }

    #[test]
    fn entry_ids_must_be_sixteen_bytes() {
        let mut data = sample_vault();
        data.entries.truncate(1);
        let mut tree = map_value(shared_fields(&data), &data.extra);
        let Value::Map(pairs) = &mut tree else {
            unreachable!()
        };
        pairs.push((text("v"), int(1)));
        let Value::Map(pairs) = &mut tree else {
            unreachable!()
        };
        let entries = pairs
            .iter_mut()
            .find(|(k, _)| *k == text("entries"))
            .unwrap();
        let Value::Array(items) = &mut entries.1 else {
            unreachable!()
        };
        let Value::Map(entry) = &mut items[0] else {
            unreachable!()
        };
        entry.iter_mut().find(|(k, _)| *k == text("id")).unwrap().1 = Value::Bytes(vec![1, 2, 3]);
        assert!(
            matches!(decode(&encoded(&tree)), Err(VaultError::InvalidPayload(m)) if m.contains("16 bytes"))
        );
    }

    #[test]
    fn duplicate_keys_are_refused() {
        let duplicated = Value::Map(vec![(text("v"), int(1)), (text("v"), int(1))]);
        assert!(
            matches!(decode(&encoded(&duplicated)), Err(VaultError::InvalidPayload(m)) if m.contains("repeats"))
        );
    }

    #[test]
    fn garbage_and_trailing_bytes_are_refused() {
        assert!(decode(&[0xff, 0xff]).is_err());
        let mut bytes = payload_bytes(base_payload());
        bytes.push(0);
        assert!(
            matches!(decode(&bytes), Err(VaultError::InvalidPayload(m)) if m.contains("trailing"))
        );
    }

    #[test]
    fn padding_reaches_a_multiple_of_the_block_size_and_is_reversible() {
        for length in [0, 1, 1022, 1023, 1024, 1025, 5000] {
            let payload = vec![0x42; length];
            let padded = pad(&payload);
            assert_eq!(padded.len() % PAD_BLOCK, 0, "length {length}");
            assert!(padded.len() > length);
            assert_eq!(
                unpad(&padded).unwrap(),
                payload.as_slice(),
                "length {length}"
            );
        }
    }

    #[test]
    fn padding_hides_small_changes_in_size() {
        assert_eq!(pad(&[1; 10]).len(), pad(&[1; 900]).len());
    }

    #[test]
    fn malformed_padding_is_refused() {
        assert!(unpad(&[]).is_err());
        assert!(unpad(&[0, 0, 0]).is_err());
        assert!(unpad(&[1, 2, 3, 0, 0]).is_err());
    }

    #[test]
    fn ids_in_tombstones_survive() {
        let data = decode(&encode(&sample_vault()).unwrap()).unwrap();
        assert_eq!(data.tombstones[0].id, id(3));
    }

    #[test]
    fn sync_settings_survive_the_local_payload() {
        let mut data = sample_vault();
        data.sync = Some(sync_state("C:/cloud/vault.mfasync"));
        assert_eq!(decode(&encode(&data).unwrap()).unwrap(), data);
    }

    #[test]
    fn the_shared_payload_never_holds_the_sync_settings() {
        let mut data = sample_vault();
        data.sync = Some(sync_state("a-very-recognisable-location"));
        let shared = encode_shared(&data, 1).unwrap();
        for private in [
            b"a-very-recognisable-location".as_slice(),
            &[0x51; 16],
            &[0x52; 32],
            &[0x53; 16],
        ] {
            assert!(!shared.windows(private.len()).any(|part| part == private));
        }
        assert_eq!(decode_shared(&shared).unwrap(), (1, sample_vault()));
    }

    #[test]
    fn a_shared_payload_carrying_sync_settings_is_refused() {
        let mut fields = shared_fields(&sample_vault());
        fields.insert(SYNC_KEY.to_owned(), sync_value(&sync_state("planted")));
        let tree = map_value(
            BTreeMap::from([
                ("v".to_owned(), int(1)),
                ("revision".to_owned(), int(1)),
                ("vault".to_owned(), map_value(fields, &Extra::new())),
            ]),
            &Extra::new(),
        );
        let result = decode_shared(&to_cbor(&tree).unwrap());
        assert!(matches!(result, Err(VaultError::InvalidPayload(_))));
    }

    #[test]
    fn a_shared_payload_needs_a_positive_revision_and_nothing_unknown_beside_the_vault() {
        let build = |revision: i64, extra: Option<&str>| {
            let mut fields = BTreeMap::from([
                ("v".to_owned(), int(1)),
                ("revision".to_owned(), int(revision)),
                (
                    "vault".to_owned(),
                    map_value(shared_fields(&sample_vault()), &Extra::new()),
                ),
            ]);
            if let Some(key) = extra {
                fields.insert(key.to_owned(), int(1));
            }
            decode_shared(&to_cbor(&map_value(fields, &Extra::new())).unwrap())
        };
        assert!(build(1, None).is_ok());
        assert!(build(0, None).is_err());
        assert!(build(-1, None).is_err());
        assert!(build(1, Some("later")).is_err());
    }
}
