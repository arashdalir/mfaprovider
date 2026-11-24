//! Opt-in check against a real export on the developer's machine. The file is read at run
//! time and never copied anywhere; only counts are printed.
//!
//! `MFA_SAMPLE_EXPORT=path/to/export.txt cargo test -p mfa-formats --test real_export_sample -- --ignored --nocapture`

use mfa_core::otp::OtpRegistry;
use mfa_core::vault::VaultData;
use mfa_formats::import::{Importer, LinkListImporter};
use mfa_formats::merge::{plan_merge, MergeStatus};

#[test]
#[ignore = "needs MFA_SAMPLE_EXPORT pointing at a real export file"]
fn a_real_link_list_export_imports_cleanly() {
    let path = std::env::var("MFA_SAMPLE_EXPORT").expect("set MFA_SAMPLE_EXPORT");
    let bytes = std::fs::read(path).unwrap();
    let batch = LinkListImporter
        .parse(&bytes, &OtpRegistry::with_builtins())
        .unwrap();
    let skipped = batch.skipped.len();
    let definitions: Vec<_> = batch.entries.into_iter().map(|e| e.definition).collect();
    let with_issuer = definitions.iter().filter(|d| !d.issuer.is_empty()).count();
    let statuses = plan_merge(&VaultData::empty(0), &definitions);
    let new = statuses.iter().filter(|s| **s == MergeStatus::New).count();
    println!(
        "accounts read: {}, skipped lines: {skipped}, new in an empty vault: {new}, with issuer: {with_issuer}",
        definitions.len()
    );
    assert_eq!(skipped, 0, "every line should be usable");
    assert_eq!(new, definitions.len());
}
