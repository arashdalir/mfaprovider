//! Proof of the no-secrets rule (ADR-0012, decision 7).

use mfa_core::otp::{OtpRegistry, Params};
use mfa_core::secret::Secret;
use mfa_core::vault::{EntryId, NewEntry, VaultData};

use super::dto::entry_views;

const SEED: &[u8] = b"\x01\x02Qz9!kLmN3pRsT7vW";

fn vault_with_known_seed() -> VaultData {
    let registry = OtpRegistry::with_builtins();
    let mut data = VaultData::empty(0);
    for (number, otp_type) in [(1_u8, "totp"), (2, "hotp"), (3, "steam")] {
        let new = NewEntry {
            otp_type: otp_type.into(),
            issuer: "Issuer".into(),
            account: format!("user{number}"),
            secret: Secret::new(SEED.to_vec()),
            params: Params::new(),
            icon: None,
            tags: vec!["tag".into()],
            notes: "note".into(),
        };
        data.add_entry(EntryId::from_bytes([number; 16]), new, &registry, 5)
            .unwrap();
    }
    data
}

fn every_encoding_of_the_seed() -> Vec<(&'static str, String)> {
    let hex: String = SEED.iter().map(|byte| format!("{byte:02x}")).collect();
    let decimals = SEED.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
    vec![
        ("base32", Secret::new(SEED.to_vec()).to_base32().to_string()),
        ("lower hex", hex.clone()),
        ("upper hex", hex.to_uppercase()),
        ("decimal array", decimals),
        ("printable part", "Qz9!kLmN3pRsT7vW".to_owned()),
    ]
}

#[test]
fn serialised_entry_lists_contain_no_seed_in_any_encoding() {
    let data = vault_with_known_seed();
    let codes = data.codes(&OtpRegistry::with_builtins(), 1_700_000_000_000);
    let json = serde_json::to_string(&entry_views(&data, codes)).unwrap();
    assert!(
        json.contains("Issuer"),
        "the sample must actually serialise"
    );
    for (name, needle) in every_encoding_of_the_seed() {
        assert!(
            !json.contains(&needle),
            "the {name} form of the seed leaked"
        );
    }
}

#[test]
fn the_encoding_check_would_catch_a_leak() {
    for (name, needle) in every_encoding_of_the_seed() {
        let leaky = format!(r#"{{"x":"{needle}"}}"#);
        assert!(leaky.contains(&needle), "{name}");
    }
}

/// Types allowed to hold a seed, and the only field name they may hold it in.
const SEED_TYPES: [&str; 3] = ["NewEntryRequest", "EntryChangesRequest", "RevealedSecret"];
const SEED_FIELD: &str = "secret_base32";
const FORBIDDEN: [&str; 4] = ["secret", "seed", "vec<u8>", "[u8"];

/// Source-level tripwire: a seed-like field may only appear as `secret_base32` inside the
/// allowed types. Any other DTO field that smells like a secret fails this test.
#[test]
fn only_the_designated_types_have_a_seed_field() {
    let source = include_str!("dto.rs");
    let (mut current, mut depth): (Option<&str>, i32) = (None, 0);
    let mut seen_in_allowed = 0;
    for line in source.lines().take_while(|l| !l.contains("#[cfg(test)]")) {
        let code = line.split("//").next().unwrap_or("").trim();
        if depth == 0 {
            current = ["pub struct ", "pub enum "]
                .iter()
                .find_map(|prefix| code.strip_prefix(prefix))
                .and_then(|rest| rest.split([' ', '{', '(']).next());
        } else if let Some(name) = current {
            let lowered = code.to_lowercase();
            if FORBIDDEN.iter().any(|word| lowered.contains(word)) {
                let allowed =
                    SEED_TYPES.contains(&name) && code.starts_with(&format!("pub {SEED_FIELD}:"));
                assert!(
                    allowed,
                    "`{name}` has a field that looks like a secret: {code}"
                );
                seen_in_allowed += 1;
            }
        }
        depth += i32::try_from(code.matches('{').count()).unwrap();
        depth -= i32::try_from(code.matches('}').count()).unwrap();
        if current.is_some() && depth == 0 && code.contains('}') {
            current = None;
        }
    }
    assert_eq!(
        seen_in_allowed, 3,
        "the scan must see the three designated fields"
    );
}
